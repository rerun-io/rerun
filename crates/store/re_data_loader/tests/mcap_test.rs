#[cfg(test)]
mod tests {
    use re_data_loader::{DataLoaderSettings, loader_mcap::load_mcap};
    use re_mcap::layers::SelectedLayers;

    #[test]
    fn test_load_simple_protobuf_mcap_file() {
        let file = include_bytes!("assets/simple-protobuf.mcap");
        let (tx, rx) = std::sync::mpsc::channel();
        let settings = DataLoaderSettings::recommended("test");
        load_mcap(file, &settings, &tx, SelectedLayers::All).unwrap();
        let res = rx.recv().unwrap();
        println!("res: {:#?}", res);
    }
}
