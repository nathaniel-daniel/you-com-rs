use crate::Error;
use nd_tokio_sse_codec::SseCodecError;
use nd_tokio_sse_codec::SseEvent;
use std::pin::Pin;
use std::task::ready;
use std::task::Context;
use std::task::Poll;
use tokio_stream::Stream;
use tokio_stream::StreamExt;

/// A Chat Message
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ChatMessage {
    /// The user question
    pub question: String,

    /// The AI response
    pub answer: String,
}

/// A chat event
#[derive(Debug)]
pub enum ChatEvent {
    /// A new token was sent.
    Token {
        /// The new token
        token: String,
    },

    /// The stream is done.
    Done {
        /// The associated text.
        text: String,
    },
}

/// The response stream for a chat.
pub struct ChatResponseStream {
    stream: Pin<Box<dyn Stream<Item = Result<SseEvent, SseCodecError>> + Send>>,
}

impl ChatResponseStream {
    /// Create a new chat response stream.
    pub(crate) fn new(
        stream: Pin<Box<dyn Stream<Item = Result<SseEvent, SseCodecError>> + Send>>,
    ) -> Self {
        Self { stream }
    }

    /// Collect the answer tokens.
    ///
    /// This will consume all events in the stream.
    /// All non-token events will be ignored.
    pub async fn collect_answer(&mut self) -> Result<String, Error> {
        let mut answer = String::new();
        while let Some(event) = self.next().await {
            let event = event?;

            if let ChatEvent::Token { token } = event {
                answer.push_str(&token);
            }
        }

        Ok(answer)
    }
}

impl std::fmt::Debug for ChatResponseStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChatResponseStream").finish()
    }
}

impl Stream for ChatResponseStream {
    type Item = Result<ChatEvent, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            let event = ready!(self.stream.as_mut().poll_next(cx));
            let event = match event {
                Some(Ok(event)) => event,
                Some(Err(error)) => {
                    return Poll::Ready(Some(Err(error.into())));
                }
                None => {
                    return Poll::Ready(None);
                }
            };

            let event = match event.event.as_deref() {
                Some("youChatToken") => {
                    let data = event.data.as_deref().ok_or(Error::SseEventMissingData)?;
                    let data: YouChatTokenEventData = serde_json::from_str(data)
                        .map_err(|error| Error::InvalidJsonEventData { error })?;

                    ChatEvent::Token {
                        token: data.you_chat_token,
                    }
                }
                Some("done") => {
                    let data = event.data.ok_or(Error::SseEventMissingData)?;
                    ChatEvent::Done { text: data }
                }
                Some(_event) => {
                    // dbg!(event);
                    continue;
                }
                None => {
                    continue;
                }
            };

            return Poll::Ready(Some(Ok(event)));
        }
    }
}

#[derive(serde::Deserialize)]
struct YouChatTokenEventData {
    #[serde(rename = "youChatToken")]
    you_chat_token: String,
}
