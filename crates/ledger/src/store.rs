use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::op::Op;
use crate::path::{ErasedPath, Field};
use crate::value::Value;

const SNAPSHOT_EVERY: u64 = 2000;
const VERSION: u64 = 1;

/// Where an app keeps its facts on this device: a snapshot, a log of every change since, and where the user was. Writing happens on Ledger's own thread, so the main thread only hands over what changed.
pub struct Store {
    send: mpsc::Sender<Message>,
    thread: Option<JoinHandle<()>>,
    trouble: Arc<Mutex<Option<String>>>,
    /// This device's id for new keys, kept from its first launch.
    pub replica: u64,
}

/// What opening a store found.
pub struct Opened<F> {
    pub facts: F,
    /// Where the user was when the app last closed, as the kernel wrote it.
    pub place: Option<Value>,
    /// The last key counter handed out, so new keys never repeat old ones.
    pub counter: u64,
    /// What went wrong while reading, if anything was lost or set aside.
    pub trouble: Option<String>,
}

enum Message {
    Entry { entry: Value, ops: Vec<Op>, counter: u64 },
    Place(Value),
    Close,
}

struct Files {
    folder: PathBuf,
    log: File,
    end: u64,
    snapshot_at: u64,
    since: u64,
    counter: u64,
}

impl Store {
    /// Opens the store in `folder`, making it if needed: loads the last snapshot, applies the log written after it and starts Ledger's thread. `fresh` makes the facts for a first launch.
    pub fn open<F: Field + Send>(folder: &Path, fresh: impl FnOnce() -> F) -> io::Result<(Store, Opened<F>)> {
        fs::create_dir_all(folder)?;
        let replica = replica(folder)?;
        let mut trouble = None;
        let (mut facts, snapshot_at, mut counter) = match fs::read(folder.join("facts")) {
            Ok(bytes) => match read_snapshot::<F>(&bytes) {
                Some(found) => found,
                None => {
                    let aside = folder.join(format!("facts unreadable {}", now_ms()));
                    fs::rename(folder.join("facts"), &aside)?;
                    let _ = fs::rename(folder.join("log"), aside.with_extension("log"));
                    trouble = Some(format!("The saved facts couldn't be read, so they were set aside in {}", aside.display()));
                    (fresh(), 0, 0)
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => (fresh(), 0, 0),
            Err(e) => return Err(e),
        };
        facts.attach(ErasedPath::ROOT);
        let log_path = folder.join("log");
        let bytes = match fs::read(&log_path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e),
        };
        let mut end = snapshot_at.min(bytes.len() as u64);
        let mut since = 0;
        for (record, next) in records(&bytes, end as usize) {
            let Some((ops, c)) = read_record(&record) else { break };
            for op in &ops {
                facts.apply(op.path().steps(), op);
            }
            counter = counter.max(c);
            end = next as u64;
            since += 1;
        }
        if end < bytes.len() as u64 && trouble.is_none() {
            trouble = Some("The last change before the app closed was cut short, so it was left out".into());
        }
        let log = OpenOptions::new().create(true).truncate(false).write(true).open(&log_path)?;
        log.set_len(end)?;
        let mut log = OpenOptions::new().append(true).open(&log_path)?;
        log.flush()?;
        let place = fs::read(folder.join("place")).ok().and_then(|b| Value::from_bytes(&b).ok());
        let mut files = Files { folder: folder.to_path_buf(), log, end, snapshot_at, since, counter };
        let mut copy = facts.clone();
        let (send, receive) = mpsc::channel();
        let shared = Arc::new(Mutex::new(None));
        let problems = shared.clone();
        let thread = std::thread::Builder::new().name("ledger".into()).spawn(move || {
            let mut place = None;
            while let Ok(first) = receive.recv() {
                let mut closing = false;
                for message in std::iter::once(first).chain(std::iter::from_fn(|| receive.try_recv().ok())) {
                    let done = match message {
                        Message::Entry { entry, ops, counter } => files.append(&mut copy, entry, ops, counter),
                        Message::Place(p) => {
                            place = Some(p);
                            Ok(())
                        }
                        Message::Close => {
                            closing = true;
                            Ok(())
                        }
                    };
                    report(&problems, done);
                }
                if files.since >= SNAPSHOT_EVERY || closing && files.since > 0 {
                    report(&problems, files.snapshot(&copy));
                }
                if let Some(p) = place.take() {
                    report(&problems, write_atomic(&files.folder.join("place"), &p.to_bytes()));
                }
                if closing {
                    break;
                }
            }
        })?;
        let store = Store { send, thread: Some(thread), trouble: shared, replica };
        Ok((store, Opened { facts, place, counter, trouble }))
    }

    /// Hands a committed entry to Ledger's thread: what the user did, the operations it made and the key counter after it.
    pub fn record(&self, entry: Value, ops: Vec<Op>, counter: u64) {
        let _ = self.send.send(Message::Entry { entry, ops, counter });
    }

    /// Keeps where the user is, written when the thread is next free.
    pub fn place(&self, place: Value) {
        let _ = self.send.send(Message::Place(place));
    }

    /// What went wrong while writing, once.
    pub fn trouble(&self) -> Option<String> {
        self.trouble.lock().ok().and_then(|mut t| t.take())
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        let _ = self.send.send(Message::Close);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Files {
    fn append<F: Field>(&mut self, facts: &mut F, entry: Value, ops: Vec<Op>, counter: u64) -> io::Result<()> {
        for op in &ops {
            facts.apply(op.path().steps(), op);
        }
        let record = Value::Map(vec![("entry".into(), entry), ("ops".into(), Value::List(ops.iter().map(Op::to_value).collect())), ("counter".into(), Value::UInt(counter))]);
        let bytes = record.to_bytes();
        let mut framed = Vec::with_capacity(bytes.len() + 8);
        framed.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        framed.extend_from_slice(&checksum(&bytes).to_le_bytes());
        framed.extend_from_slice(&bytes);
        self.log.write_all(&framed)?;
        self.end += framed.len() as u64;
        self.since += 1;
        self.counter = self.counter.max(counter);
        Ok(())
    }

    fn snapshot<F: Field>(&mut self, facts: &F) -> io::Result<()> {
        self.log.flush()?;
        let snapshot = Value::Map(vec![("version".into(), Value::UInt(VERSION)), ("log".into(), Value::UInt(self.end)), ("counter".into(), Value::UInt(self.counter)), ("facts".into(), facts.value())]);
        write_atomic(&self.folder.join("facts"), &snapshot.to_bytes())?;
        self.snapshot_at = self.end;
        self.since = 0;
        Ok(())
    }
}

fn report(problems: &Mutex<Option<String>>, done: io::Result<()>) {
    if let (Err(e), Ok(mut slot)) = (done, problems.lock()) {
        *slot = Some(format!("Changes couldn't be saved: {e}"));
    }
}

fn read_snapshot<F: Field>(bytes: &[u8]) -> Option<(F, u64, u64)> {
    let value = Value::from_bytes(bytes).ok()?;
    let (Some(Value::UInt(VERSION)), Some(Value::UInt(log)), Some(Value::UInt(counter)), Some(facts)) = (value.field("version"), value.field("log"), value.field("counter"), value.field("facts")) else { return None };
    Some((F::from_value(facts)?, *log, *counter))
}

fn read_record(record: &[u8]) -> Option<(Vec<Op>, u64)> {
    let value = Value::from_bytes(record).ok()?;
    let (Some(Value::List(ops)), Some(Value::UInt(counter))) = (value.field("ops"), value.field("counter")) else { return None };
    Some((ops.iter().map(Op::from_value).collect::<Option<Vec<_>>>()?, *counter))
}

/// The whole records of a log from a byte offset, each with the offset after it. A record cut short or damaged ends the log there.
fn records(bytes: &[u8], from: usize) -> impl Iterator<Item = (Vec<u8>, usize)> + '_ {
    let mut at = from;
    std::iter::from_fn(move || {
        let head = bytes.get(at..at + 8)?;
        let len = u32::from_le_bytes(head[..4].try_into().ok()?) as usize;
        let sum = u32::from_le_bytes(head[4..].try_into().ok()?);
        let body = bytes.get(at + 8..at + 8 + len)?;
        if checksum(body) != sum {
            return None;
        }
        at += 8 + len;
        Some((body.to_vec(), at))
    })
}

/// Reads every entry in a store's log, oldest first: the command log, for replays and for showing what the user did.
pub fn entries(folder: &Path) -> io::Result<Vec<Value>> {
    let bytes = fs::read(folder.join("log"))?;
    Ok(records(&bytes, 0).filter_map(|(r, _)| Value::from_bytes(&r).ok()?.field("entry").cloned()).collect())
}

fn checksum(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5u32, |h, b| (h ^ *b as u32).wrapping_mul(0x0100_0193))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = path.with_extension("new");
    let mut file = File::create(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, path)
}

fn replica(folder: &Path) -> io::Result<u64> {
    let path = folder.join("replica");
    if let Ok(bytes) = fs::read(&path)
        && let Ok(bytes) = <[u8; 8]>::try_from(bytes.as_slice())
    {
        return Ok(u64::from_le_bytes(bytes));
    }
    let id = fresh_replica();
    write_atomic(&path, &id.to_le_bytes())?;
    Ok(id)
}

/// A new replica id, from the time and the process, never one `Key::fixed` uses.
pub fn fresh_replica() -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in now_ms().to_le_bytes().into_iter().chain(std::process::id().to_le_bytes()) {
        h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
    }
    h | 1 << 63
}

fn now_ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos())
}
