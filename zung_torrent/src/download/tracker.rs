use crate::trackers::TrackersList;

use super::Downloader;

pub struct TrackerDownloader<T = UnAnnounced> {
    trackers: TrackersList,
    state: T,
}

pub struct UnAnnounced;

impl TrackerDownloader {
    pub fn new(trackers: TrackersList) -> TrackerDownloader<UnAnnounced> {
        TrackerDownloader {
            trackers,
            state: UnAnnounced,
        }
    }
}

impl<T> Downloader for TrackerDownloader<T> {
    fn download_all(&self) {
        todo!()
    }
}
