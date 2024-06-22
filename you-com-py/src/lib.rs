use anyhow::Context;
use once_cell::sync::Lazy;
use pyo3::exceptions::PyIndexError;
use pyo3::prelude::*;
use pyo3::types::PyString;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::RwLockReadGuard;
use tokio_stream::StreamExt;
use you_com::Client;

static TOKIO_RUNTIME: Lazy<std::io::Result<tokio::runtime::Runtime>> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
});

static CLIENT: Lazy<Client> = Lazy::new(Client::new);

/// A chat with an AI
#[pyclass(sequence)]
struct Chat {
    chat: Arc<RwLock<Vec<you_com::ChatMessage>>>,
}

impl Chat {
    /// Get the chat, if it is not busy.
    fn get_chat_ref(&self) -> Option<RwLockReadGuard<'_, Vec<you_com::ChatMessage>>> {
        self.chat.try_read().ok()
    }
}

#[pymethods]
impl Chat {
    /// Make a new chat.
    #[new]
    pub fn new() -> Self {
        Self {
            chat: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get the number of chat messages.
    pub fn __len__(&self) -> PyResult<usize> {
        let chat = self.get_chat_ref().context("chat is busy")?;
        Ok(chat.len())
    }

    /// Get a copy of the chat message at the given index.
    pub fn __getitem__(&self, mut index: isize) -> PyResult<ChatMessage> {
        let chat = self.get_chat_ref().context("chat is busy")?;

        let chat_len = chat.len();
        if index < 0 {
            index += isize::try_from(chat_len)?;
        }
        let index = match usize::try_from(index) {
            Ok(index) => index,
            Err(_err) => {
                return Err(PyIndexError::new_err("message index out of range"));
            }
        };

        let message = match chat.get(index) {
            Some(message) => message,
            None => {
                return Err(PyIndexError::new_err("message index out of range"));
            }
        };

        Ok(ChatMessage {
            question: message.question.clone(),
            answer: message.answer.clone(),
        })
    }

    /// Create a message and get the response.
    pub fn send_message(&self, py: Python<'_>, question: String) -> PyResult<ChatResponseStream> {
        py.allow_threads(|| {
            let tokio_rt = TOKIO_RUNTIME
                .as_ref()
                .context("failed to init tokio runtime")?;

            let mut chat = self
                .chat
                .clone()
                .try_write_owned()
                .ok()
                .context("chat is busy")?;

            let rx = tokio_rt.block_on(async move {
                let mut stream = CLIENT.chat(&question, &chat).await?;

                let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                tokio::spawn(async move {
                    let mut answer = String::new();
                    let mut stream_error = None;

                    while let Some(event) = stream.next().await {
                        let event = match event.context("invalid event") {
                            Ok(event) => event,
                            Err(error) => {
                                stream_error = Some(error);
                                break;
                            }
                        };

                        if let you_com::ChatEvent::Token { token } = event {
                            answer.push_str(&token);
                            let _ = tx.send(Ok(token)).is_ok();
                        }
                    }

                    match stream_error {
                        Some(error) => {
                            let _ = tx.send(Err(error)).is_ok();
                        }
                        None => {
                            chat.push(you_com::ChatMessage { question, answer });
                        }
                    }
                });

                anyhow::Ok(rx)
            })?;

            Ok(ChatResponseStream { rx })
        })
    }

    pub fn __str__(&self) -> String {
        let chat = self.get_chat_ref();
        match chat {
            Some(chat) => format!("Chat(messages={:?})", chat),
            None => "Chat(<chat is busy>)".into(),
        }
    }
}

/// A chat message
#[pyclass]
struct ChatMessage {
    /// The chat question
    pub question: String,

    /// The chat answer
    pub answer: String,
}

#[pymethods]
impl ChatMessage {
    /// Make a new chat message.
    #[new]
    pub fn new(question: String, answer: String) -> Self {
        Self { question, answer }
    }

    /// Get the question.
    pub fn question(&self) -> &str {
        self.question.as_str()
    }

    /// Get the answer.
    pub fn answer(&self) -> &str {
        self.answer.as_str()
    }

    pub fn __str__(&self) -> String {
        format!(
            "ChatMessage(question=\"{}\", answer=\"{}\")",
            self.question, self.answer
        )
    }
}

/// A streaming chat response.
#[pyclass]
pub struct ChatResponseStream {
    rx: tokio::sync::mpsc::UnboundedReceiver<anyhow::Result<String>>,
}

#[pymethods]
impl ChatResponseStream {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__<'a>(&mut self, py: Python<'a>) -> PyResult<Option<Bound<'a, PyString>>> {
        py.allow_threads(|| self.rx.blocking_recv().transpose())
            .map(|token| token.map(|token| PyString::new_bound(py, &token)))
            .map_err(Into::into)
    }
}

/// A Python module for you.com, in Rust.
#[pymodule]
fn you_com_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Chat>()?;
    m.add_class::<ChatMessage>()?;
    m.add_class::<ChatResponseStream>()?;
    Ok(())
}
