use std::collections::HashMap;

use re_protos::cloud::v1alpha1::index_properties::Props;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{
    CreateIndexRequest, DeleteIndexesRequest, IndexColumn, IndexConfig, IndexProperties,
    InvertedIndex, ListIndexesRequest, SearchDatasetRequest, VectorIvfPqIndex,
};
use re_protos::common::v1alpha1::{ComponentDescriptor, EntityPath, IndexColumnSelector, Timeline};
use re_protos::headers::RerunHeadersInjectorExt as _;

use super::common::{DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _};

// --- Tests ---

/// Goes through the entire lifecycle of an index: creation, listing, search, deletion.
pub async fn index_lifecycle(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::scalars("my_segment_id1").layer_name("scalars"), //
            LayerDefinition::text("my_segment_id1").layer_name("text"),       //
            LayerDefinition::embeddings("my_segment_id1", 256, 3).layer_name("embeddings"), //
        ],
    );

    let dataset_name = "my_dataset1";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    let indexes = list_indexes(&service, dataset_name).await.unwrap();
    assert!(indexes.is_empty());

    for req in generate_search_dataset_requests() {
        let code = service
            .search_dataset(
                tonic::Request::new(req)
                    .with_entry_name(dataset_name)
                    .unwrap(),
            )
            .await
            .map(|_| ())
            .unwrap_err()
            .code();
        // TODO(RR-2779): OSS returns NotFound.
        // This is more precise and Rerun Cloud should be updated to return it.
        assert!(code == tonic::Code::InvalidArgument || code == tonic::Code::NotFound);
    }

    // TODO(cmc): At some point we will want to properly define what happens in case of concurrent
    // creations/deletions/listings, but we're not quite there yet.
    for _ in 0..3 {
        for req in generate_create_index_requests() {
            create_index(&service, dataset_name, req).await.unwrap();
        }

        for req in generate_create_index_requests() {
            assert!({
                let code = service
                    .create_index(
                        tonic::Request::new(req)
                            .with_entry_name(dataset_name)
                            .unwrap(),
                    )
                    .await
                    .unwrap_err()
                    .code();

                // TODO(RR-2779): OSS returns AlreadyExists.
                // This is more precise and Rerun Cloud should be updated to return it.
                code == tonic::Code::InvalidArgument || code == tonic::Code::AlreadyExists
            });
        }

        let expected_indexes: HashMap<IndexColumn, IndexConfig> = generate_create_index_requests()
            .into_iter()
            .map(|index| {
                let config = index.config.unwrap();
                (config.column.clone().unwrap(), config)
            })
            .collect();

        let indexes = list_indexes(&service, dataset_name).await.unwrap();
        assert_eq!(expected_indexes, indexes);

        for req in generate_search_dataset_requests() {
            search_dataset(&service, dataset_name, req).await.unwrap();
        }

        let mut search_dataset_requests: HashMap<IndexColumn, SearchDatasetRequest> =
            generate_search_dataset_requests()
                .into_iter()
                .map(|req| (req.column.clone().unwrap(), req))
                .collect();
        for (column, config) in expected_indexes {
            let deleted_indexes = delete_indexes(
                &service,
                dataset_name,
                DeleteIndexesRequest {
                    column: Some(column.clone()),
                },
            )
            .await
            .unwrap();

            assert!(deleted_indexes.len() == 1);
            assert_eq!(config, deleted_indexes.into_values().next().unwrap());

            let indexes = list_indexes(&service, dataset_name).await.unwrap();
            assert!(!indexes.contains_key(&column));

            let code = service
                .search_dataset(
                    tonic::Request::new(search_dataset_requests.remove(&column).unwrap())
                        .with_entry_name(dataset_name)
                        .unwrap(),
                )
                .await
                .map(|_| ())
                .unwrap_err()
                .code();
            // TODO(RR-2779): OSS returns NotFound.
            // This is more precise and Rerun Cloud should be updated to return it.
            assert!(code == tonic::Code::InvalidArgument || code == tonic::Code::NotFound);

            for req in search_dataset_requests.values() {
                search_dataset(&service, dataset_name, req.clone())
                    .await
                    .unwrap();
            }
        }

        let indexes = list_indexes(&service, dataset_name).await.unwrap();
        assert!(indexes.is_empty());
    }
}

pub async fn dataset_doesnt_exist(service: impl RerunCloudService) {
    let dataset_name = "doesnt_exist";

    let create_index_request = generate_create_index_requests().into_iter().next().unwrap();
    let search_dataset_request = generate_search_dataset_requests()
        .into_iter()
        .next()
        .unwrap();

    let code = service
        .list_indexes(
            tonic::Request::new(ListIndexesRequest {})
                .with_entry_name(dataset_name)
                .unwrap(),
        )
        .await
        .unwrap_err()
        .code();

    assert_eq!(code, tonic::Code::NotFound);

    let code = service
        .search_dataset(
            tonic::Request::new(search_dataset_request)
                .with_entry_name(dataset_name)
                .unwrap(),
        )
        .await
        .map(|_| ())
        .unwrap_err()
        .code();
    assert_eq!(code, tonic::Code::NotFound);

    let code = service
        .create_index(
            tonic::Request::new(create_index_request.clone())
                .with_entry_name(dataset_name)
                .unwrap(),
        )
        .await
        .unwrap_err()
        .code();
    assert_eq!(code, tonic::Code::NotFound);

    let code = service
        .delete_indexes(
            tonic::Request::new(DeleteIndexesRequest {
                column: create_index_request.config.unwrap().column,
            })
            .with_entry_name(dataset_name)
            .unwrap(),
        )
        .await
        .unwrap_err()
        .code();
    assert_eq!(code, tonic::Code::NotFound);
}

pub async fn column_doesnt_exist(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::scalars("my_segment_id1").layer_name("scalars"), //
            LayerDefinition::text("my_segment_id1").layer_name("text"),       //
            LayerDefinition::embeddings("my_segment_id1", 256, 3).layer_name("embeddings"), //
        ],
    );

    let dataset_name = "my_dataset1";
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    let mut create_index_requests = generate_create_index_requests();
    for req in &mut create_index_requests {
        let entity_path = &mut req
            .config
            .as_mut()
            .unwrap()
            .column
            .as_mut()
            .unwrap()
            .entity_path
            .as_mut()
            .unwrap()
            .path;

        *entity_path = "doesnt_exist".to_owned();
    }

    let mut search_dataset_requests = generate_search_dataset_requests();
    for req in &mut search_dataset_requests {
        let entity_path = &mut req
            .column
            .as_mut()
            .unwrap()
            .entity_path
            .as_mut()
            .unwrap()
            .path;

        *entity_path = "doesnt_exist".to_owned();
    }

    let indexes = list_indexes(&service, dataset_name).await.unwrap();
    assert!(indexes.is_empty());

    for req in search_dataset_requests {
        let code = service
            .search_dataset(
                tonic::Request::new(req)
                    .with_entry_name(dataset_name)
                    .unwrap(),
            )
            .await
            .map(|_| ())
            .unwrap_err()
            .code();

        // TODO(RR-3100)
        assert!(code == tonic::Code::InvalidArgument || code == tonic::Code::NotFound);
    }

    for req in &create_index_requests {
        let deleted_indexes = delete_indexes(
            &service,
            dataset_name,
            DeleteIndexesRequest {
                column: req.config.clone().unwrap().column,
            },
        )
        .await
        .unwrap();

        assert!(deleted_indexes.is_empty());
    }

    for req in &create_index_requests {
        let code = service
            .create_index(
                tonic::Request::new(req.clone())
                    .with_entry_name(dataset_name)
                    .unwrap(),
            )
            .await
            .unwrap_err()
            .code();

        // TODO(RR-3100)
        assert!(code == tonic::Code::InvalidArgument || code == tonic::Code::NotFound);
    }
}

// --- Helpers ---

/// Generates a bunch of [`CreateIndexRequest`]s for every kind of index.
fn generate_create_index_requests() -> Vec<CreateIndexRequest> {
    vec![
        // scalars / btree
        CreateIndexRequest {
            config: Some(IndexConfig {
                properties: Some(IndexProperties {
                    props: Some(Props::Btree(re_protos::cloud::v1alpha1::BTreeIndex {})),
                }),
                time_index: Some(IndexColumnSelector {
                    timeline: Some(Timeline {
                        name: "log_time".to_owned(),
                    }),
                }),
                column: Some(IndexColumn {
                    entity_path: Some(EntityPath {
                        path: "/my_scalars".to_owned(),
                    }),
                    component: Some(ComponentDescriptor {
                        component: Some("scalar".to_owned()),
                        ..Default::default()
                    }),
                }),
            }),
        },
        // text / fts
        CreateIndexRequest {
            config: Some(IndexConfig {
                properties: Some(IndexProperties {
                    props: Some(Props::Inverted(InvertedIndex {
                        store_position: Some(false),
                        base_tokenizer: Some("simple".to_owned()),
                    })),
                }),
                time_index: Some(IndexColumnSelector {
                    timeline: Some(Timeline {
                        name: "log_time".to_owned(),
                    }),
                }),
                column: Some(IndexColumn {
                    entity_path: Some(EntityPath {
                        path: "/my_text".to_owned(),
                    }),
                    component: Some(ComponentDescriptor {
                        component_type: Some("rerun.components.Text".to_owned()),
                        archetype: Some("rerun.archetypes.TextLog".to_owned()),
                        component: Some("TextLog:text".to_owned()),
                    }),
                }),
            }),
        },
        // embeddings / vector
        CreateIndexRequest {
            config: Some(IndexConfig {
                properties: Some(IndexProperties {
                    props: Some(Props::Vector(VectorIvfPqIndex {
                        target_partition_num_rows: Some(128),
                        num_sub_vectors: Some(16),
                        distance_metrics: re_protos::cloud::v1alpha1::VectorDistanceMetric::L2
                            as i32,
                    })),
                }),
                time_index: Some(IndexColumnSelector {
                    timeline: Some(Timeline {
                        name: "log_time".to_owned(),
                    }),
                }),
                column: Some({
                    IndexColumn {
                        entity_path: Some(EntityPath {
                            path: "/my_embeddings".to_owned(),
                        }),
                        component: Some(ComponentDescriptor {
                            archetype: None,
                            component: Some("embedding".to_owned()),
                            component_type: None,
                        }),
                    }
                }),
            }),
        },
    ]
}

/// Generates a bunch of [`SearchDatasetRequest`]s for every kind of index.
fn generate_search_dataset_requests() -> Vec<SearchDatasetRequest> {
    use std::sync::Arc;

    use arrow::array::{Float32Array, RecordBatch, StringArray};
    use arrow::datatypes::Field;
    use re_protos::cloud::v1alpha1::index_query_properties::Props;
    use re_protos::cloud::v1alpha1::{
        BTreeIndexQuery, IndexQueryProperties, InvertedIndexQuery, VectorIndexQuery,
    };

    let mut create_index_requests = generate_create_index_requests().into_iter();
    vec![
        // scalars / btree
        SearchDatasetRequest {
            column: create_index_requests.next().unwrap().config.unwrap().column,
            query: Some(
                RecordBatch::try_new(
                    Arc::new(arrow::datatypes::Schema::new(vec![Field::new(
                        "query",
                        arrow::datatypes::DataType::Utf8,
                        false,
                    )])),
                    vec![Arc::new(StringArray::from(vec!["42.0"]))],
                )
                .unwrap()
                .into(),
            ),
            properties: Some(IndexQueryProperties {
                props: Some(Props::Btree(BTreeIndexQuery {})),
            }),
            scan_parameters: None,
        },
        // text / fts
        SearchDatasetRequest {
            column: create_index_requests.next().unwrap().config.unwrap().column,
            query: Some(
                RecordBatch::try_new(
                    Arc::new(arrow::datatypes::Schema::new(vec![Field::new(
                        "query",
                        arrow::datatypes::DataType::Utf8,
                        false,
                    )])),
                    vec![Arc::new(StringArray::from(vec!["the wind cries mary"]))],
                )
                .unwrap()
                .into(),
            ),
            properties: Some(IndexQueryProperties {
                props: Some(Props::Inverted(InvertedIndexQuery {})),
            }),
            scan_parameters: None,
        },
        // embeddings / vector
        SearchDatasetRequest {
            column: create_index_requests.next().unwrap().config.unwrap().column,
            query: Some(
                RecordBatch::try_new(
                    Arc::new(arrow::datatypes::Schema::new(vec![Field::new(
                        "query",
                        arrow::datatypes::DataType::Float32,
                        false,
                    )])),
                    vec![Arc::new(Float32Array::from_iter_values(
                        (0..256).map(|_| 42.0f32),
                    ))],
                )
                .unwrap()
                .into(),
            ),
            properties: Some(IndexQueryProperties {
                props: Some(
                    re_protos::cloud::v1alpha1::index_query_properties::Props::Vector(
                        VectorIndexQuery { top_k: Some(5) },
                    ),
                ),
            }),
            scan_parameters: None,
        },
    ]
}

/// Returns `Ok(())` if the operation is not supported.
async fn create_index(
    service: &impl RerunCloudService,
    dataset_name: &str,
    req: CreateIndexRequest,
) -> tonic::Result<()> {
    let _res = service
        .create_index(tonic::Request::new(req).with_entry_name(dataset_name)?)
        .await?;

    Ok(())
}

async fn search_dataset(
    service: &impl RerunCloudService,
    dataset_name: &str,
    req: SearchDatasetRequest,
) -> tonic::Result<()> {
    let _res = service
        .search_dataset(tonic::Request::new(req).with_entry_name(dataset_name)?)
        .await?;

    // Results are ignored. This is not about testing the search itself, it's about testing the
    // lifecycle of the underlying index.
    Ok(())
}

async fn list_indexes(
    service: &impl RerunCloudService,
    dataset_name: &str,
) -> tonic::Result<HashMap<IndexColumn, IndexConfig>> {
    let res = service
        .list_indexes(tonic::Request::new(ListIndexesRequest {}).with_entry_name(dataset_name)?)
        .await?;

    let indexes: HashMap<IndexColumn, IndexConfig> = res
        .into_inner()
        .indexes
        .into_iter()
        .map(|config| (config.column.clone().unwrap(), config))
        .collect();

    Ok(indexes)
}

async fn delete_indexes(
    service: &impl RerunCloudService,
    dataset_name: &str,
    req: DeleteIndexesRequest,
) -> tonic::Result<HashMap<IndexColumn, IndexConfig>> {
    let res = service
        .delete_indexes(tonic::Request::new(req).with_entry_name(dataset_name)?)
        .await?;

    let indexes: HashMap<IndexColumn, IndexConfig> = res
        .into_inner()
        .indexes
        .into_iter()
        .map(|config| (config.column.clone().unwrap(), config))
        .collect();

    Ok(indexes)
}
