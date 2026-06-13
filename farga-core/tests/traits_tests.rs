use farga_core::reader::FargaReader;
use farga_core::writer::FargaWriter;

// These just verify the traits compile and are object-safe
fn _assert_reader_object_safe(_: &dyn FargaReader) {}
fn _assert_writer_object_safe(_: &dyn FargaWriter) {}

#[test]
fn traits_are_object_safe() {
    // If this test compiles, the traits are object-safe.
}
