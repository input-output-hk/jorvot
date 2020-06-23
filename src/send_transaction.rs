use iced_futures::futures;
use serde::Deserialize;

// Just a little utility function
pub fn post<T: ToString>(url: T, body: Box<[u8]>) -> iced::Subscription<Progress> {
    iced::Subscription::from_recipe(Download {
        url: url.to_string(),
        body,
    })
}

pub struct Download {
    url: String,
    body: Box<[u8]>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SendTransactionState(pub String);

// Make sure iced can use our download stream
impl<H, I> iced_native::subscription::Recipe<H, I> for Download
where
    H: std::hash::Hasher,
{
    type Output = Progress;

    fn hash(&self, state: &mut H) {
        use std::hash::Hash;

        std::any::TypeId::of::<Self>().hash(state);
        self.url.hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: futures::stream::BoxStream<'static, I>,
    ) -> futures::stream::BoxStream<'static, Self::Output> {
        Box::pin(futures::stream::unfold(
            State::Ready(self.url, self.body),
            |state| async move {
                match state {
                    State::Ready(url, body) => {
                        let client = reqwest::Client::new();
                        let response = client.post(&url)
                            .header("Content-Type", "application/octet-stream")
                            .body(Vec::from(body)).send().await;

                        match response {
                            Ok(response) => {
                                if let Some(total) = response.content_length() {
                                    Some((
                                        Progress::Started,
                                        State::Downloading {
                                            response,
                                            total,
                                            downloaded: 0,
                                            bytes: Vec::with_capacity(total as usize),
                                        },
                                    ))
                                } else {
                                    Some((
                                        Progress::Errored {
                                            status_code: response.status(),
                                        },
                                        State::Finished,
                                    ))
                                }
                            }
                            Err(error) => Some((
                                Progress::Failure {
                                    error: error.to_string(),
                                },
                                State::Finished,
                            )),
                        }
                    }
                    State::Downloading {
                        mut response,
                        total,
                        downloaded,
                        mut bytes,
                    } => {
                        dbg!(response.status());
                        if response.status() != reqwest::StatusCode::OK {
                            return Some((
                                Progress::Errored {
                                    status_code: response.status(),
                                },
                                State::Finished,
                            ));
                        }
                        match response.chunk().await {
                            Ok(Some(chunk)) => {
                                let downloaded = downloaded + chunk.len() as u64;

                                let percentage = (downloaded as f32 / total as f32) * 100.0;

                                bytes.extend_from_slice(&chunk);

                                Some((
                                    Progress::Advanced(percentage),
                                    State::Downloading {
                                        response,
                                        total,
                                        downloaded,
                                        bytes,
                                    },
                                ))
                            }
                            Ok(None) => {
                                match serde_json::from_slice::<SendTransactionState>(&bytes) {
                                    Err(error) => Some((
                                        Progress::Failure {
                                            error: error.to_string(),
                                        },
                                        State::Finished,
                                    )),
                                    Ok(state) => {
                                        Some((Progress::Finished { state }, State::Finished))
                                    }
                                }
                            }
                            Err(error) => Some((
                                Progress::Failure {
                                    error: error.to_string(),
                                },
                                State::Finished,
                            )),
                        }
                    }
                    State::Finished => {
                        dbg!();
                        // We do not let the stream die, as it would start a
                        // new download repeatedly if the user is not careful
                        // in case of errors.
                        let _: () = iced::futures::future::pending().await;

                        None
                    }
                }
            },
        ))
    }
}

#[derive(Debug, Clone)]
pub enum Progress {
    Started,
    Advanced(f32),
    Finished { state: SendTransactionState },
    Errored { status_code: reqwest::StatusCode },
    Failure { error: String },
}

pub enum State {
    Ready(String, Box<[u8]>),
    Downloading {
        response: reqwest::Response,
        total: u64,
        downloaded: u64,
        bytes: Vec<u8>,
    },
    Finished,
}
