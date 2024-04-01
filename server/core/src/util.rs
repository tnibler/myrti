pub trait OptionPathExt {
    fn as_opt_path(&self) -> Option<&camino::Utf8Path>;
}

impl OptionPathExt for Option<camino::Utf8PathBuf> {
    fn as_opt_path(&self) -> Option<&camino::Utf8Path> {
        self.as_ref().map(|p| p.as_path())
    }
}
