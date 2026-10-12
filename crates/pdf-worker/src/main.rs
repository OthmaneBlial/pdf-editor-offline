fn main() {
    if let Err(error) = pdf_worker::serve_once(pdf_worker::document::execute_document) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
