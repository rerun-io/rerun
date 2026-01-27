from __future__ import annotations

from typing import TYPE_CHECKING

from rerun._arrow import to_record_batch

if TYPE_CHECKING:
    from uuid import UUID

    import datafusion
    import pyarrow as pa

    from rerun_bindings import ViewerClientInternal


class ViewerClient:
    """
    A connection to an instance of a Rerun viewer.

    !!! warning
        This API is experimental and may change or be removed in future versions.
    """

    def __init__(self, addr: str = "127.0.0.1:9876") -> None:
        """
        Create a new viewer client connection.

        Parameters
        ----------
        addr : str
            The address of the viewer to connect to, in the format "host:port".
            Defaults to "127.0.0.1:9876" for a local viewer.

        """
        from rerun_bindings import ViewerClientInternal

        self._internal: ViewerClientInternal = ViewerClientInternal(addr)

    def send_table(self, name: str, table: pa.RecordBatch | list[pa.RecordBatch] | datafusion.DataFrame) -> None:
        """
        Send a table to the viewer.

        A table is represented as a dataframe defined by an Arrow record batch.

        Parameters
        ----------
        name:
            The table name.

            !!! note
                The table name serves as an identifier.
                If you send a table with the same name twice, the second table will replace the first one.

        table:
            The Arrow RecordBatch containing the table data to send.

        """
        # TODO(RR-3481): we should be able to stream multiple record batches instead of having to merge to one. This
        # requires changing the grpc protocol though, or rolling a OSS server sidecar to the Viewer.
        self._internal.send_table(name, to_record_batch(table))

    def save_screenshot(self, file_path: str, view_id: str | UUID | None = None) -> None:
        """
        Save a screenshot to a file.

        !!! warning
            This API is experimental and may change or be removed in future versions.

        Parameters
        ----------
        file_path:
            The path where the screenshot will be saved.

            !!! important
                This path is relative to the viewer's filesystem, not the client's.
                If your viewer runs on a different machine, the screenshot will be saved there.

        view_id:
            Optional view ID to screenshot.
            If None, screenshots the entire viewer.

        """
        view_id_str = str(view_id) if view_id is not None else None
        self._internal.save_screenshot(file_path, view_id_str)
