//! Native smoke recipe for the shared durable command path, without an editor UI.
//! Run with an explicit local directory; no default location or network is used.
#[cfg(not(target_arch = "wasm32"))]
mod native {
    use kortex_contract::v2::Contract;
    use kortex_platform::projects::{NativeStore, StoreError};
    use kortex_project::{
        Change, Command, Grants, Project, Source,
        persistence::{DurableSession, PersistenceError},
    };
    use kortex_workflow::project::{Catalogue, Plan, Step};
    use std::{
        future::Future,
        path::PathBuf,
        task::{Context, Poll, Waker},
    };

    struct Tools(Contract);
    impl Catalogue for Tools {
        fn project_contract(&self, tool: &str) -> Option<&Contract> {
            (tool == self.0.tool.id).then_some(&self.0)
        }
    }

    // NativeStore performs blocking local I/O and completes without yielding. A GUI host
    // should await it on its background executor instead of using this tiny CLI adapter.
    fn complete_native(future: impl Future<Output = Result<(), String>>) -> Result<(), String> {
        let mut context = Context::from_waker(Waker::noop());
        let mut future = std::pin::pin!(future);
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => value,
            Poll::Pending => {
                Err("the native recipe requires a background executor for yielding storage".into())
            }
        }
    }

    fn command(revision: u64, source: Source, key: &str, value: &str) -> Command {
        Command {
            id: format!("example-{revision}"),
            project_id: "durable-example".into(),
            expected_revision: revision,
            operation: "edit".into(),
            source,
            changes: vec![Change::SetProperty {
                key: key.into(),
                value: value.into(),
            }],
        }
    }

    pub fn run() -> Result<(), String> {
        let mut args = kortex_platform::native::env::args().into_iter();
        let root = PathBuf::from(
            args.next()
                .ok_or("usage: durable_project <explicit-local-directory>")?,
        );
        if args.next().is_some() {
            return Err("usage: durable_project <explicit-local-directory>".into());
        }
        complete_native(async move {
            let legacy =
                kortex_contract::parse(include_str!("../../../tools/text/reverse/tool.toml"))?;
            let mut contract = Contract::from_v1(&legacy).map_err(|error| format!("{error:?}"))?;
            contract.operations[0].id = "edit".into();
            contract.operations[0].permissions.clipboard = "none".into();
            contract.operations[0].effects.mutates_project = true;
            contract.operations[0].effects.undoable = true;
            let tools = Tools(contract);
            let store = NativeStore::open(&root).map_err(|error| format!("{error:?}"))?;
            let mut session =
                match DurableSession::load(store.clone(), "durable-example", 32, 1024 * 1024).await
                {
                    Ok(session) => session,
                    Err(PersistenceError::NotFound) => DurableSession::create(
                        store.clone(),
                        Project::new("durable-example".into()),
                        32,
                        1024 * 1024,
                    )
                    .await
                    .map_err(|error| format!("{error:?}"))?,
                    Err(error) => return Err(format!("{error:?}")),
                };
            let mut stale = DurableSession::load(store, "durable-example", 32, 1024 * 1024)
                .await
                .map_err(|error| format!("{error:?}"))?;
            let revision = session.project().revision;
            if revision > u64::MAX - 3 {
                return Err("project revision exhausted".into());
            }
            let grants = Grants::default();
            session
                .apply(
                    command(revision, Source::Ui, "title", "Shared local project"),
                    &tools.0.operations[0],
                    &grants,
                )
                .await
                .map_err(|error| format!("{error:?}"))?;
            let stale_plan = Plan {
                schema: 1,
                steps: vec![Step {
                    tool: legacy.tool.id.clone(),
                    command: command(revision, Source::Flow, "stale", "Must be rejected"),
                }],
            };
            if stale_plan
                .execute_persisted(&tools, &mut stale, &grants)
                .await
                != Err(kortex_workflow::project::Error::Persistence {
                    step: 0,
                    cause: PersistenceError::Store(StoreError::Conflict),
                })
            {
                return Err("stale writer was not rejected".into());
            }
            drop(stale);
            let plan = Plan {
                schema: 1,
                steps: vec![
                    Step {
                        tool: legacy.tool.id.clone(),
                        command: command(revision + 1, Source::Flow, "preset", "vertical"),
                    },
                    Step {
                        tool: legacy.tool.id,
                        command: command(revision + 2, Source::Mind, "note", "Ready for review"),
                    },
                ],
            };
            plan.execute_persisted(&tools, &mut session, &grants)
                .await
                .map_err(|error| format!("{error:?}"))?;
            let expected = session.project().clone();
            drop(session);
            let reopened = DurableSession::load(
                NativeStore::open(&root).map_err(|error| format!("{error:?}"))?,
                "durable-example",
                32,
                1024 * 1024,
            )
            .await
            .map_err(|error| format!("{error:?}"))?;
            if reopened.project() != &expected {
                return Err("reopened project differs from the committed snapshot".into());
            }
            println!(
                "Saved and reopened revision {} in {}. Stale writer rejected. UI, Flow and Mind use the same validated commands.",
                reopened.project().revision,
                root.display()
            );
            Ok(())
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    match native::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}
