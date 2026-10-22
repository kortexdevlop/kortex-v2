//! Cooperative lifecycle model. `CancelRequested` is not proof a worker has stopped.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Queued,
    Running,
    CancelRequested,
    Cancelled,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidTransition,
    InvalidProgress,
    NotCancellable,
}

#[derive(Debug)]
pub struct Job {
    state: State,
    progress: u16,
    cancellable: bool,
}

impl Job {
    pub fn new(cancellable: bool) -> Self {
        Self {
            state: State::Queued,
            progress: 0,
            cancellable,
        }
    }
    pub fn state(&self) -> State {
        self.state
    }
    /// Basis points (0..=10_000), monotonic during execution.
    pub fn progress(&self) -> u16 {
        self.progress
    }
    pub fn start(&mut self) -> Result<(), Error> {
        if self.state != State::Queued {
            return Err(Error::InvalidTransition);
        }
        self.state = State::Running;
        Ok(())
    }
    pub fn report(&mut self, progress: u16) -> Result<(), Error> {
        if self.state != State::Running {
            return Err(Error::InvalidTransition);
        }
        if progress < self.progress || progress > 10_000 {
            return Err(Error::InvalidProgress);
        }
        self.progress = progress;
        Ok(())
    }
    pub fn request_cancel(&mut self) -> Result<(), Error> {
        match self.state {
            State::Queued => self.state = State::Cancelled,
            State::Running if self.cancellable => self.state = State::CancelRequested,
            State::Running => return Err(Error::NotCancellable),
            _ => return Err(Error::InvalidTransition),
        }
        Ok(())
    }
    /// Called only after the backend has stopped and released its resources.
    pub fn acknowledge_cancel(&mut self) -> Result<(), Error> {
        if self.state != State::CancelRequested {
            return Err(Error::InvalidTransition);
        }
        self.state = State::Cancelled;
        Ok(())
    }
    pub fn finish(&mut self, success: bool) -> Result<(), Error> {
        if self.state != State::Running {
            return Err(Error::InvalidTransition);
        }
        self.state = if success {
            State::Succeeded
        } else {
            State::Failed
        };
        if success {
            self.progress = 10_000;
        }
        Ok(())
    }
}
