// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use std::{
    io::{self, Cursor, Read},
    os::unix::process::ExitStatusExt as _,
    pin::Pin,
    process::{Command, Output},
};

use tokio::io::AsyncRead;
use tokio_stream::once;
use tokio_util::io::StreamReader;

// Used to execute commands on the filesystem.
pub struct LocalCmd;

pub struct Child {
    pub stderr: Vec<u8>,
    pub stdout: Vec<u8>,
}

pub trait ChildLike: Send {
    fn stdout(&mut self) -> Option<Box<dyn Read + Send>>;
    fn stderr(&mut self) -> Option<Box<dyn Read + Send>>;
    fn wait(&mut self) -> std::io::Result<std::process::ExitStatus>;
}

impl ChildLike for std::process::Child {
    fn stdout(&mut self) -> Option<Box<dyn Read + Send>> {
        self.stdout
            .take()
            .map(|x| Box::new(x) as Box<dyn Read + Send>)
    }

    fn stderr(&mut self) -> Option<Box<dyn Read + Send>> {
        self.stderr
            .take()
            .map(|x| Box::new(x) as Box<dyn Read + Send>)
    }

    fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        self.wait()
    }
}

impl ChildLike for Child {
    fn stdout(&mut self) -> Option<Box<dyn Read + Send>> {
        Some(Box::new(Cursor::new(self.stdout.clone())))
    }

    fn stderr(&mut self) -> Option<Box<dyn Read + Send>> {
        let cursor = Cursor::new(self.stderr.clone());
        Some(Box::new(cursor))
    }

    fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        Ok(std::process::ExitStatus::from_raw(0))
    }
}

pub trait RemoteCmd: Send + Sync {
    /// Get output of a remote command. All implementations of this trait *must*
    /// return `Ok(Output)` on non-zero Exit code.
    fn output_unchecked(&self, cmd: &mut Command) -> io::Result<Output>;

    /// Get output of a remote command. Returns `Err(io::Error)` on non-zero Exit code.
    fn output(&self, cmd: &mut Command) -> io::Result<Output> {
        let x = self.output_unchecked(cmd)?;

        if x.status.success() {
            Ok(x)
        } else {
            Err(io::Error::other(format!("{x:?}")))
        }
    }

    // Spawn a remote command.
    fn spawn(&self, cmd: &mut Command) -> io::Result<Box<dyn ChildLike>> {
        let child = cmd.spawn()?;

        Ok(Box::new(child))
    }
}

impl RemoteCmd for LocalCmd {
    fn output_unchecked(&self, cmd: &mut Command) -> io::Result<Output> {
        cmd.output()
    }
}

#[async_trait::async_trait]
pub trait ChildLikeAsync: Send {
    fn stdout(&mut self) -> Option<Pin<Box<dyn AsyncRead + Send>>>;
    fn stderr(&mut self) -> Option<Pin<Box<dyn AsyncRead + Send>>>;
    async fn wait(&mut self) -> std::io::Result<std::process::ExitStatus>;
}

#[async_trait::async_trait]
impl ChildLikeAsync for tokio::process::Child {
    fn stdout(&mut self) -> Option<Pin<Box<dyn AsyncRead + Send>>> {
        self.stdout.take().map(|s| Box::pin(s) as _)
    }

    fn stderr(&mut self) -> Option<Pin<Box<dyn AsyncRead + Send>>> {
        self.stderr.take().map(|s| Box::pin(s) as _)
    }

    async fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        tokio::process::Child::wait(&mut *self).await
    }
}

#[async_trait::async_trait]
impl ChildLikeAsync for Child {
    fn stdout(&mut self) -> Option<Pin<Box<dyn AsyncRead + Send>>> {
        let cursor = Cursor::new(self.stdout.clone());
        let stream = once(Ok::<_, std::io::Error>(cursor));
        let reader = StreamReader::new(stream);

        Some(Box::pin(reader))
    }

    fn stderr(&mut self) -> Option<Pin<Box<dyn AsyncRead + Send>>> {
        let cursor = Cursor::new(self.stderr.clone());

        Some(Box::pin(cursor))
    }

    async fn wait(&mut self) -> io::Result<std::process::ExitStatus> {
        Ok(std::process::ExitStatus::from_raw(0))
    }
}

#[async_trait::async_trait]
pub trait RemoteCmdAsync: Send + Sync {
    /// Get output of a remote command. All implementations of this trait *must*
    /// return `Ok(Output)` on non-zero Exit code.
    async fn output_unchecked(&self, cmd: &mut tokio::process::Command) -> io::Result<Output>;

    /// Get output of a remote command. Returns `Err(io::Error)` on non-zero Exit code.
    async fn output(&self, cmd: &mut tokio::process::Command) -> io::Result<Output> {
        let x = self.output_unchecked(cmd).await?;

        if x.status.success() {
            Ok(x)
        } else {
            Err(io::Error::other(format!("{x:?}")))
        }
    }

    // Spawn a remote command.
    async fn spawn(
        &self,
        cmd: &mut tokio::process::Command,
    ) -> io::Result<Box<dyn ChildLikeAsync>> {
        let child = cmd.spawn()?;

        Ok(Box::new(child))
    }
}

#[async_trait::async_trait]
impl RemoteCmdAsync for LocalCmd {
    async fn output_unchecked(&self, cmd: &mut tokio::process::Command) -> io::Result<Output> {
        cmd.output().await
    }
}

pub mod test_utils {
    use crate::remote_cmd::{Child, ChildLike, ChildLikeAsync, RemoteCmd, RemoteCmdAsync};
    use std::{
        ffi::OsStr,
        io, iter,
        process::{Command, Output},
        time::Duration,
    };

    /// Test only struct for mocking remote commands.
    ///
    /// Anything that is an [`Iterator`] of [`Output`] can be used to mock remote commands via the [`TestCmd::set`] call.
    ///
    /// The [`Iterator`] will move forward on each call to the matching command. Once it reaches the end, an
    /// Error will be returned on the next call.
    #[derive(Default)]
    pub struct TestCmd {
        output_matches:
            dashmap::DashMap<String, Box<dyn Iterator<Item = (Output, Duration)> + Send + Sync>>,
        spawn_matches: dashmap::DashMap<
            &'static str,
            Box<dyn Iterator<Item = (Child, Duration)> + Send + Sync>,
        >,
    }

    pub fn get_command_parts(cmd: &Command) -> String {
        iter::once(cmd.get_program())
            .chain(cmd.get_args())
            .map(OsStr::to_string_lossy)
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn get_command_parts_async(cmd: &tokio::process::Command) -> String {
        let cmd = cmd.as_std();

        iter::once(cmd.get_program())
            .chain(cmd.get_args())
            .map(OsStr::to_string_lossy)
            .collect::<Vec<_>>()
            .join(" ")
    }

    impl TestCmd {
        pub fn set_output<I>(self, k: String, v: I) -> Self
        where
            I: IntoIterator<Item = Output> + 'static,
            I::IntoIter: Iterator<Item = Output> + Send + Sync + 'static,
        {
            self.set_output_with_duration(k, v.into_iter().map(|x| (x, Duration::from_secs(0))))
        }

        pub fn set_output_with_duration<I>(self, k: String, v: I) -> Self
        where
            I: IntoIterator<Item = (Output, Duration)> + 'static,
            I::IntoIter: Iterator<Item = (Output, Duration)> + Send + Sync + 'static,
        {
            self.output_matches.insert(k, Box::new(v.into_iter()));

            self
        }

        pub fn set_spawn<I>(self, k: &'static str, v: I) -> Self
        where
            I: IntoIterator<Item = Child> + 'static,
            I::IntoIter: Iterator<Item = Child> + Send + Sync + 'static,
        {
            self.set_spawn_with_duration(k, v.into_iter().map(|x| (x, Duration::from_secs(0))))
        }

        pub fn set_spawn_with_duration<I>(self, k: &'static str, v: I) -> Self
        where
            I: IntoIterator<Item = (Child, Duration)> + 'static,
            I::IntoIter: Iterator<Item = (Child, Duration)> + Send + Sync + 'static,
        {
            self.spawn_matches.insert(k, Box::new(v.into_iter()));

            self
        }

        fn evaluate_cmd_output(&self, cmd: &Command) -> Result<(Output, Duration), io::Error> {
            let cmd = get_command_parts(cmd);

            let Some(mut xs) = dashmap::DashMap::get_mut(&self.output_matches, &cmd) else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Unexpected command: {cmd}"),
                ));
            };

            let Some(x) = xs.next() else {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("No more output for cmd: {cmd}"),
                ));
            };

            Ok(x)
        }

        fn evaluate_async_cmd_output(
            &self,
            cmd: &tokio::process::Command,
        ) -> Result<(Output, Duration), io::Error> {
            let cmd = get_command_parts_async(cmd);

            let Some(mut xs) = dashmap::DashMap::get_mut(&self.output_matches, &cmd) else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Unexpected command: {cmd}"),
                ));
            };

            let Some(x) = xs.next() else {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("No more output for cmd: {cmd}"),
                ));
            };

            Ok(x)
        }

        fn evaluate_cmd_spawn(&self, cmd: &Command) -> Result<(Child, Duration), io::Error> {
            let cmd = get_command_parts(cmd);

            let Some(mut xs) = dashmap::DashMap::get_mut(&self.spawn_matches, cmd.as_str()) else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Unexpected command: {cmd}"),
                ));
            };

            let Some(x) = xs.next() else {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("No more output for cmd: {cmd}"),
                ));
            };

            Ok(x)
        }

        fn evaluate_async_cmd_spawn(
            &self,
            cmd: &tokio::process::Command,
        ) -> Result<(Child, Duration), io::Error> {
            let cmd = get_command_parts_async(cmd);

            let Some(mut xs) = dashmap::DashMap::get_mut(&self.spawn_matches, cmd.as_str()) else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Unexpected command: {cmd}"),
                ));
            };

            let Some(x) = xs.next() else {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("No more output for cmd: {cmd}"),
                ));
            };

            Ok(x)
        }
    }

    impl RemoteCmd for TestCmd {
        fn output_unchecked(&self, cmd: &mut Command) -> io::Result<Output> {
            let (result, duration) = self.evaluate_cmd_output(cmd)?;

            std::thread::sleep(duration);

            Ok(result)
        }

        fn spawn(&self, cmd: &mut Command) -> io::Result<Box<dyn ChildLike>> {
            let (result, duration) = self.evaluate_cmd_spawn(cmd)?;

            std::thread::sleep(duration);

            Ok(Box::new(result))
        }
    }

    #[async_trait::async_trait]
    impl RemoteCmdAsync for TestCmd {
        async fn output_unchecked(&self, cmd: &mut tokio::process::Command) -> io::Result<Output> {
            let (result, duration) = self.evaluate_async_cmd_output(cmd)?;

            tokio::time::sleep(duration).await;

            Ok(result)
        }

        async fn spawn(
            &self,
            cmd: &mut tokio::process::Command,
        ) -> io::Result<Box<dyn ChildLikeAsync>> {
            let (result, duration) = self.evaluate_async_cmd_spawn(cmd)?;

            tokio::time::sleep(duration).await;

            Ok(Box::new(result))
        }
    }

    impl std::fmt::Display for TestCmd {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "TestCmd")
        }
    }
}
