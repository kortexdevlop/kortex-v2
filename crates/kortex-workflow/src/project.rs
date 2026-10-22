//! Additive project-plan executor. Existing v1 graphs and file-byte runners are unchanged.
//! Each command is atomic; a plan stops on failure and retains earlier committed commands.
use kortex_contract::v2::{Contract, Operation};
use kortex_project::{Command, Grants, Receipt, Session};
use serde::{Deserialize, Serialize};

pub trait Catalogue {
    fn project_contract(&self, tool: &str) -> Option<&Contract>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub tool: String,
    pub command: Command,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: u32,
    pub steps: Vec<Step>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    InvalidPlan,
    UnknownOperation {
        step: usize,
    },
    Command {
        step: usize,
        cause: kortex_project::Error,
    },
    #[cfg(feature = "project-persistence")]
    Persistence {
        step: usize,
        cause: kortex_project::persistence::PersistenceError,
    },
}

impl Plan {
    fn resolve<'a>(&self, catalogue: &'a dyn Catalogue) -> Result<Vec<&'a Operation>, Error> {
        if self.schema != 1 || self.steps.is_empty() || self.steps.len() > 1024 {
            return Err(Error::InvalidPlan);
        }
        let mut operations = Vec::with_capacity(self.steps.len());
        for (index, step) in self.steps.iter().enumerate() {
            let contract = catalogue
                .project_contract(&step.tool)
                .ok_or(Error::UnknownOperation { step: index })?;
            if contract.tool.id != step.tool || !contract.validate().is_empty() {
                return Err(Error::InvalidPlan);
            }
            let operation = contract
                .operation(&step.command.operation)
                .filter(|op| op.effects.mutates_project)
                .ok_or(Error::UnknownOperation { step: index })?;
            operations.push(operation);
        }
        Ok(operations)
    }

    /// No model-provided permission values are accepted. Grants come from the trusted host.
    pub fn execute(
        &self,
        catalogue: &dyn Catalogue,
        session: &mut Session,
        grants: &Grants,
    ) -> Result<Vec<Receipt>, Error> {
        // Resolve the entire plan before applying any changes.
        let operations = self.resolve(catalogue)?;
        let mut receipts = Vec::with_capacity(self.steps.len());
        for (index, (step, operation)) in self.steps.iter().zip(operations).enumerate() {
            let receipt = session
                .apply(step.command.clone(), operation, grants)
                .map_err(|cause| Error::Command { step: index, cause })?;
            receipts.push(receipt);
        }
        Ok(receipts)
    }

    /// Resolve every operation first, then commit commands individually through the durable
    /// session. A failed step leaves earlier commits intact and stops all subsequent steps.
    /// Host-issued grants and revision checks are identical to interactive project commands.
    #[cfg(feature = "project-persistence")]
    pub async fn execute_persisted<S: kortex_platform::projects::SnapshotStore>(
        &self,
        catalogue: &dyn Catalogue,
        session: &mut kortex_project::persistence::DurableSession<S>,
        grants: &Grants,
    ) -> Result<Vec<Receipt>, Error> {
        let operations = self.resolve(catalogue)?;
        let mut receipts = Vec::with_capacity(self.steps.len());
        for (index, (step, operation)) in self.steps.iter().zip(operations).enumerate() {
            let receipt = session
                .apply(step.command.clone(), operation, grants)
                .await
                .map_err(|cause| Error::Persistence { step: index, cause })?;
            receipts.push(receipt);
        }
        Ok(receipts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kortex_project::{Change, Project, Source};

    struct Tools(Contract);
    impl Catalogue for Tools {
        fn project_contract(&self, tool: &str) -> Option<&Contract> {
            (self.0.tool.id == tool).then_some(&self.0)
        }
    }

    #[test]
    fn workflow_dispatches_versioned_project_commands_and_rejects_unknown_steps_before_mutation() {
        let legacy =
            kortex_contract::parse(include_str!("../../../tools/text/reverse/tool.toml")).unwrap();
        let mut contract = Contract::from_v1(&legacy).unwrap();
        contract.operations[0].id = "edit".into();
        contract.operations[0].permissions.clipboard = "none".into();
        contract.operations[0].effects.mutates_project = true;
        contract.operations[0].effects.undoable = true;
        let tools = Tools(contract);
        let command = Command {
            id: "command-1".into(),
            project_id: "project-1".into(),
            expected_revision: 0,
            operation: "edit".into(),
            source: Source::Flow,
            changes: vec![Change::SetProperty {
                key: "title".into(),
                value: "Workflow".into(),
            }],
        };
        let mut plan = Plan {
            schema: 1,
            steps: vec![Step {
                tool: legacy.tool.id,
                command,
            }],
        };
        let wire = serde_json::to_string(&plan).unwrap();
        let decoded: Plan = serde_json::from_str(&wire).unwrap();
        let mut session = Session::open(Project::new("project-1".into()), 8, 4096).unwrap();
        let mut invalid = plan.steps[0].clone();
        invalid.command.operation = "unknown".into();
        plan.steps.push(invalid);
        assert_eq!(
            plan.execute(&tools, &mut session, &Grants::default()),
            Err(Error::UnknownOperation { step: 1 })
        );
        assert_eq!(session.project().revision, 0);
        let receipts = decoded
            .execute(&tools, &mut session, &Grants::default())
            .unwrap();
        assert_eq!(receipts[0].revision, 1);
        assert_eq!(session.project().properties["title"], "Workflow");
    }
}

#[cfg(all(test, feature = "project-persistence", not(target_arch = "wasm32")))]
mod persisted_tests {
    use super::*;
    use kortex_platform::projects::{NativeStore, StoreError};
    use kortex_project::{
        Change, Project, Source,
        persistence::{DurableSession, PersistenceError},
    };
    use std::{
        future::Future,
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
        task::{Context, Poll, Wake, Waker},
    };

    struct ThreadWake(std::thread::Thread);
    impl Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    fn block_on<T>(future: impl Future<Output = T>) -> T {
        let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
        let mut context = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::park(),
            }
        }
    }

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            Self(std::env::temp_dir().join(format!(
                "kortex-workflow-durable-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
        }
        fn store(&self) -> NativeStore {
            NativeStore::open(&self.0).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    struct Tools(Contract);
    impl Catalogue for Tools {
        fn project_contract(&self, tool: &str) -> Option<&Contract> {
            (self.0.tool.id == tool).then_some(&self.0)
        }
    }
    fn tools() -> Tools {
        let legacy =
            kortex_contract::parse(include_str!("../../../tools/text/reverse/tool.toml")).unwrap();
        let mut contract = Contract::from_v1(&legacy).unwrap();
        let operation = &mut contract.operations[0];
        operation.id = "edit".into();
        operation.permissions.clipboard = "none".into();
        operation.effects.mutates_project = true;
        operation.effects.undoable = true;
        Tools(contract)
    }
    fn step(revision: u64, name: &str) -> Step {
        Step {
            tool: "text.reverse".into(),
            command: Command {
                id: name.into(),
                project_id: "workflow-project".into(),
                expected_revision: revision,
                operation: "edit".into(),
                source: Source::Flow,
                changes: vec![Change::SetProperty {
                    key: name.into(),
                    value: name.into(),
                }],
            },
        }
    }
    async fn create(fixture: &Fixture) -> DurableSession<NativeStore> {
        DurableSession::create(
            fixture.store(),
            Project::new("workflow-project".into()),
            16,
            65536,
        )
        .await
        .unwrap()
    }
    async fn reopen(fixture: &Fixture) -> DurableSession<NativeStore> {
        DurableSession::load(fixture.store(), "workflow-project", 16, 65536)
            .await
            .unwrap()
    }

    #[test]
    fn durable_workflow_commits_each_step_and_reopens_the_same_project() {
        block_on(async {
            let fixture = Fixture::new();
            let tools = tools();
            let mut session = create(&fixture).await;
            let mut second = step(1, "second");
            second.command.source = Source::Mind;
            let plan = Plan {
                schema: 1,
                steps: vec![step(0, "first"), second],
            };
            let receipts = plan
                .execute_persisted(&tools, &mut session, &Grants::default())
                .await
                .unwrap();
            assert_eq!(
                receipts.iter().map(|r| r.revision).collect::<Vec<_>>(),
                vec![1, 2]
            );
            assert_eq!(session.project(), reopen(&fixture).await.project());
        });
    }

    #[test]
    fn durable_workflow_resolves_the_entire_plan_before_writing() {
        block_on(async {
            let fixture = Fixture::new();
            let tools = tools();
            let mut session = create(&fixture).await;
            let mut second = step(1, "second");
            second.command.operation = "unknown".into();
            let plan = Plan {
                schema: 1,
                steps: vec![step(0, "first"), second],
            };
            assert_eq!(
                plan.execute_persisted(&tools, &mut session, &Grants::default())
                    .await,
                Err(Error::UnknownOperation { step: 1 })
            );
            assert_eq!(reopen(&fixture).await.project().revision, 0);
        });
    }

    #[test]
    fn durable_workflow_stops_after_revision_error_and_retains_prior_commit() {
        block_on(async {
            let fixture = Fixture::new();
            let tools = tools();
            let mut session = create(&fixture).await;
            let plan = Plan {
                schema: 1,
                steps: vec![step(0, "first"), step(0, "stale"), step(1, "never")],
            };
            assert_eq!(
                plan.execute_persisted(&tools, &mut session, &Grants::default())
                    .await,
                Err(Error::Persistence {
                    step: 1,
                    cause: PersistenceError::Project(kortex_project::Error::RevisionConflict)
                })
            );
            let reopened = reopen(&fixture).await;
            assert_eq!(reopened.project().revision, 1);
            assert_eq!(reopened.project().properties.len(), 1);
            assert_eq!(reopened.project().properties["first"], "first");
        });
    }

    #[test]
    fn durable_workflow_uses_host_grants_and_stops_on_permission_denial() {
        block_on(async {
            let fixture = Fixture::new();
            let mut tools = tools();
            let mut session = create(&fixture).await;
            let mut sensitive = tools.0.operations[0].clone();
            sensitive.id = "sensitive_edit".into();
            sensitive.permissions.sensitive = true;
            tools.0.operations.push(sensitive);
            let mut second = step(1, "denied");
            second.command.operation = "sensitive_edit".into();
            let plan = Plan {
                schema: 1,
                steps: vec![step(0, "first"), second, step(1, "never")],
            };
            assert_eq!(
                plan.execute_persisted(&tools, &mut session, &Grants::default())
                    .await,
                Err(Error::Persistence {
                    step: 1,
                    cause: PersistenceError::Project(kortex_project::Error::PermissionDenied)
                })
            );
            assert_eq!(reopen(&fixture).await.project().properties.len(), 1);
        });
    }

    #[test]
    fn durable_workflow_rejects_a_stale_writer_and_reopens_committed_state() {
        block_on(async {
            let fixture = Fixture::new();
            let tools = tools();
            let mut winner = create(&fixture).await;
            let mut stale = reopen(&fixture).await;
            Plan {
                schema: 1,
                steps: vec![step(0, "winner")],
            }
            .execute_persisted(&tools, &mut winner, &Grants::default())
            .await
            .unwrap();
            let result = Plan {
                schema: 1,
                steps: vec![step(0, "loser"), step(1, "never")],
            }
            .execute_persisted(&tools, &mut stale, &Grants::default())
            .await;
            assert_eq!(
                result,
                Err(Error::Persistence {
                    step: 0,
                    cause: PersistenceError::Store(StoreError::Conflict)
                })
            );
            assert!(stale.needs_reload());
            assert_eq!(stale.project().revision, 0);
            assert_eq!(reopen(&fixture).await.project(), winner.project());
        });
    }
}
