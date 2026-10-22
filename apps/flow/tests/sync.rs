//! End-to-end encrypted sync of the library against an in-memory server.
use std::cell::RefCell;
use std::collections::BTreeMap;

use kortex_flow::engine::Workflow;
use kortex_flow::library::Library;
use kortex_flow::sync::{Item, Job, Put, Remote, State, SyncError, Transport, sync};
use kortex_runtime::block_on;
use kortex_vault::Vault;
use serde_json::Value as Json;

const ROUNDS: u32 = 1_000;

#[derive(Default)]
struct Server {
    items: RefCell<BTreeMap<String, (u64, Json)>>,
    down: RefCell<Option<SyncError>>,
}

impl Transport for Server {
    async fn list(&self) -> Result<Vec<Remote>, SyncError> {
        if let Some(e) = self.down.borrow().clone() {
            return Err(e);
        }
        Ok(self
            .items
            .borrow()
            .iter()
            .map(|(k, (v, _))| Remote {
                key: k.clone(),
                version: *v,
            })
            .collect())
    }
    async fn get(&self, key: &str) -> Result<Option<Item>, SyncError> {
        Ok(self.items.borrow().get(key).map(|(v, d)| Item {
            version: *v,
            data: d.clone(),
        }))
    }
    async fn put(&self, key: &str, data: Json, expected: u64) -> Result<Put, SyncError> {
        let mut items = self.items.borrow_mut();
        let current = items.get(key).map_or(0, |(v, _)| *v);
        if current != expected {
            return Ok(Put::Conflict(items.get(key).map(|(v, d)| Item {
                version: *v,
                data: d.clone(),
            })));
        }
        items.insert(key.to_owned(), (current + 1, data));
        Ok(Put::Done(current + 1))
    }
}

/// One device: a library, its sync state and the key it derived.
struct Device {
    library: Library,
    state: State,
    cache: Option<(String, Vault)>,
    seed: u8,
}

impl Device {
    fn new(seed: u8) -> Self {
        Self {
            library: Library::default(),
            state: State::default(),
            cache: None,
            seed,
        }
    }
    fn save(&mut self, id: &str, name: &str, now: u64) {
        let w = Workflow {
            name: name.into(),
            ..Workflow::default()
        };
        self.library.save(Some(id), name, &w, now, || id.to_owned());
    }
    fn sync(
        &mut self,
        server: &Server,
        passphrase: &str,
        now: u64,
    ) -> Result<kortex_flow::sync::Report, SyncError> {
        let mut counter = self.seed;
        let mut random = move |bytes: &mut [u8]| {
            for b in bytes.iter_mut() {
                counter = counter.wrapping_mul(31).wrapping_add(17);
                *b = counter;
            }
            Ok(())
        };
        let job = Job {
            passphrase,
            rounds: ROUNDS,
            now,
            random: &mut random,
        };
        let result = block_on(sync(
            server,
            job,
            &mut self.cache,
            &mut self.library,
            &mut self.state,
        ));
        self.seed = counter_after(self.seed);
        result
    }
    fn names(&self) -> Vec<String> {
        let mut n: Vec<String> = self
            .library
            .visible()
            .iter()
            .map(|s| s.name.clone())
            .collect();
        n.sort();
        n
    }
}
fn counter_after(seed: u8) -> u8 {
    seed.wrapping_add(101)
}

#[test]
fn the_first_device_creates_the_vault_and_the_server_only_holds_ciphertext() {
    let server = Server::default();
    let mut a = Device::new(1);
    a.save("w1", "Facture clients ultra secrète", 100);
    a.save("w2", "Second", 110);
    let report = a.sync(&server, "phrase de passe", 1_000).expect("sync");
    assert_eq!((report.pulled, report.pushed), (0, 2));
    let stored = serde_json::to_string(&*server.items.borrow()).expect("json");
    assert!(
        stored.contains("flow.vault") && stored.contains("flow.w1") && stored.contains("flow.w2")
    );
    assert!(
        !stored.contains("Facture") && !stored.contains("secrète") && !stored.contains("Second"),
        "no plaintext on the server"
    );
    // Nothing changed: the next sync does nothing.
    assert_eq!(
        a.sync(&server, "phrase de passe", 2_000).expect("again"),
        Default::default()
    );
}

#[test]
fn a_second_device_with_the_same_passphrase_gets_everything_and_changes_travel_both_ways() {
    let server = Server::default();
    let mut a = Device::new(1);
    a.save("w1", "Un", 100);
    a.save("w2", "Deux", 110);
    a.sync(&server, "pp", 1_000).expect("a");
    let mut b = Device::new(2);
    let report = b.sync(&server, "pp", 1_100).expect("b");
    assert_eq!((report.pulled, report.pushed), (2, 0));
    assert_eq!(b.names(), ["Deux", "Un"]);
    // An edit on B reaches A; a deletion on A reaches B.
    b.save("w1", "Un (modifié)", 1_200);
    assert_eq!(b.sync(&server, "pp", 1_300).expect("b push").pushed, 1);
    assert_eq!(a.sync(&server, "pp", 1_400).expect("a pull").pulled, 1);
    assert_eq!(a.names(), ["Deux", "Un (modifié)"]);
    a.library.remove("w2", 1_500);
    a.sync(&server, "pp", 1_600).expect("a delete");
    b.sync(&server, "pp", 1_700).expect("b pull");
    assert_eq!(b.names(), ["Un (modifié)"], "the deletion arrived");
}

#[test]
fn a_wrong_passphrase_is_refused_before_anything_is_read_or_written() {
    let server = Server::default();
    let mut a = Device::new(1);
    a.save("w1", "Un", 100);
    a.sync(&server, "bonne", 1_000).expect("a");
    let before = serde_json::to_string(&*server.items.borrow()).expect("json");
    let mut thief = Device::new(9);
    thief.save("w9", "Mon propre workflow", 50);
    assert_eq!(
        thief.sync(&server, "mauvaise", 2_000),
        Err(SyncError::WrongPassphrase)
    );
    assert_eq!(
        thief.names(),
        ["Mon propre workflow"],
        "the local library is untouched"
    );
    assert_eq!(
        serde_json::to_string(&*server.items.borrow()).expect("json"),
        before,
        "nothing was written"
    );
}

#[test]
fn two_devices_editing_offline_converge_on_the_newest_change() {
    let server = Server::default();
    let mut a = Device::new(1);
    a.save("w1", "Départ", 100);
    a.sync(&server, "pp", 1_000).expect("a");
    let mut b = Device::new(2);
    b.sync(&server, "pp", 1_100).expect("b");
    // Both edit the same workflow while offline; B's edit is the later one.
    a.save("w1", "Édité sur A", 2_000);
    b.save("w1", "Édité sur B", 3_000);
    a.sync(&server, "pp", 4_000).expect("a first");
    let report = b.sync(&server, "pp", 4_100).expect("b second");
    assert!(report.conflicts >= 1 || report.pulled >= 1);
    a.sync(&server, "pp", 4_200).expect("a again");
    assert_eq!(a.names(), ["Édité sur B"]);
    assert_eq!(
        b.names(),
        ["Édité sur B"],
        "the newest change wins everywhere"
    );
    // The reverse order gives the same result: the older edit never overwrites the newer one.
    let server = Server::default();
    let (mut a, mut b) = (Device::new(3), Device::new(4));
    a.save("w1", "Départ", 100);
    a.sync(&server, "pp", 1_000).expect("a");
    b.sync(&server, "pp", 1_100).expect("b");
    a.save("w1", "Édité sur A", 2_000);
    b.save("w1", "Édité sur B", 3_000);
    b.sync(&server, "pp", 4_000).expect("b first");
    a.sync(&server, "pp", 4_100).expect("a second");
    b.sync(&server, "pp", 4_200).expect("b again");
    assert_eq!(
        (a.names(), b.names()),
        (
            vec!["Édité sur B".to_owned()],
            vec!["Édité sur B".to_owned()]
        )
    );
}

#[test]
fn server_problems_and_damaged_data_are_reported_without_changing_anything() {
    let server = Server::default();
    let mut a = Device::new(1);
    a.save("w1", "Un", 100);
    *server.down.borrow_mut() = Some(SyncError::Unauthenticated);
    assert_eq!(
        a.sync(&server, "pp", 1_000),
        Err(SyncError::Unauthenticated)
    );
    assert_eq!(a.state, State::default(), "a failed sync leaves no trace");
    *server.down.borrow_mut() = None;
    a.sync(&server, "pp", 2_000)
        .expect("works once the server is back");
    // A damaged item is Corrupt, not a wrong passphrase and not garbage.
    server
        .items
        .borrow_mut()
        .get_mut("flow.w1")
        .expect("item")
        .1 = serde_json::json!({"c": "v1.000000000000000000000000.deadbeef"});
    server
        .items
        .borrow_mut()
        .get_mut("flow.w1")
        .expect("item")
        .0 += 1;
    let mut b = Device::new(2);
    assert_eq!(b.sync(&server, "pp", 3_000), Err(SyncError::Corrupt));
}

#[test]
fn unavailable_randomness_never_creates_a_vault() {
    let server = Server::default();
    let mut random = |_: &mut [u8]| Err(SyncError::Server("random_unavailable".into()));
    let mut cache = None;
    let mut library = Library::default();
    let mut state = State::default();
    let result = block_on(sync(
        &server,
        Job {
            passphrase: "test",
            rounds: ROUNDS,
            now: 1,
            random: &mut random,
        },
        &mut cache,
        &mut library,
        &mut state,
    ));
    assert_eq!(result, Err(SyncError::Server("random_unavailable".into())));
    assert!(server.items.borrow().is_empty());
    assert!(cache.is_none());
    assert_eq!(state.last_sync, 0);
}

#[test]
fn unavailable_nonce_never_pushes_an_existing_vault_item() {
    let server = Server::default();
    let mut device = Device::new(1);
    device.sync(&server, "test", 1).expect("create vault");
    device.save("wlocal", "local", 2);
    let before = server.items.borrow().clone();
    let mut random = |_: &mut [u8]| Err(SyncError::Server("random_unavailable".into()));
    let result = block_on(sync(
        &server,
        Job {
            passphrase: "test",
            rounds: ROUNDS,
            now: 2,
            random: &mut random,
        },
        &mut device.cache,
        &mut device.library,
        &mut device.state,
    ));
    assert_eq!(result, Err(SyncError::Server("random_unavailable".into())));
    assert_eq!(*server.items.borrow(), before);
    assert_eq!(device.state.last_sync, 1);
}
