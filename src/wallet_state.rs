use iced_futures::futures;
use serde::Deserialize;

// Just a little utility function
pub fn query<T: ToString>(url: T) -> iced::Subscription<Progress> {
    iced::Subscription::from_recipe(Download {
        url: url.to_string(),
    })
}

pub struct Download {
    url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountState {
    pub value: u64,
    pub counter: u32,
}

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
            State::Ready(self.url),
            |state| async move {
                match state {
                    State::Ready(url) => {
                        let response = reqwest::get(&url).await;

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
                            Ok(None) => match serde_json::from_slice::<AccountState>(&bytes) {
                                Err(error) => Some((
                                    Progress::Failure {
                                        error: error.to_string(),
                                    },
                                    State::Finished,
                                )),
                                Ok(account_state) => {
                                    Some((Progress::Finished { account_state }, State::Finished))
                                }
                            },
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
    Finished { account_state: AccountState },
    Errored { status_code: reqwest::StatusCode },
    Failure { error: String },
}

pub enum State {
    Ready(String),
    Downloading {
        response: reqwest::Response,
        total: u64,
        downloaded: u64,
        bytes: Vec<u8>,
    },
    Finished,
}
