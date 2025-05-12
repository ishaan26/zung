use crate::http_seeders::HttpSeedersList;
use std::sync::Arc;

#[derive(Debug)]
pub struct HttpSeederDownloader<T> {
    pub(crate) _state: T,
    pub(crate) _list: Arc<HttpSeedersList>,
}
