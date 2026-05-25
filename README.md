# 🔴 Radish

**Radish** is a lightweight, blazing-fast, in-memory key-value data store written in **Rust**. It implements a faithful subset of the [REdis Serialization Protocol (RESP)](https://redis.io/docs/reference/protocol-spec/) and mirrors the single-threaded, asynchronous concurrency model that makes Redis itself so performant — all without a single Mutex in sight.

> Built as a learning project to deeply understand Redis internals, async Rust, and network protocol design from scratch.

---

## ✨ Features

### Core Data Operations
| Command | Syntax | Description |
|---------|--------|-------------|
| **SET** | `SET key value [EX seconds \| PX milliseconds]` | Store a value under a key, with optional TTL |
| **GET** | `GET key` | Retrieve the value for a key (returns `nil` if missing or expired) |
| **TTL** | `TTL key` | Query remaining time-to-live of a key (in seconds) |

### Time-To-Live (TTL) Support
- Attach expiry durations to any key via `EX` (seconds) or `PX` (milliseconds) on `SET`.
- `TTL` reports remaining seconds, `-1` for keys with no expiry, and `-2` for non-existent/expired keys.
- **Lazy expiration**: expired keys become invisible on access — no background threads or timers required.

### Connection Utilities
| Command | Syntax | Description |
|---------|--------|-------------|
| **PING** | `PING [message]` | Returns `PONG` or echoes the argument back |
| **ECHO** | `ECHO message` | Echoes the given message back to the client |

### Protocol Compatibility
- Fully compatible with `redis-cli` and any RESP-speaking client.
- Parses **Simple Strings** (`+`), **Bulk Strings** (`$`), **Arrays** (`*`), **Integers** (`:`), and **Errors** (`-`) natively.
- Returns proper RESP-encoded responses: simple strings, bulk strings, null bulk strings, integers, and errors.
- Gracefully handles unknown commands with informative error messages.

---

## 🏗️ Architecture

### Concurrency Model — Single-Threaded Async

Radish deliberately avoids multi-threaded complexity. Like the original Redis, it runs an **event loop on a single thread**, handling thousands of concurrent connections through asynchronous I/O.

```
┌─────────────────────────────────────────────────┐
│              Tokio (current_thread)              │
│  ┌───────────────────────────────────────────┐   │
│  │             task::LocalSet                │   │
│  │                                           │   │
│  │   ┌──────────┐  ┌──────────┐  ┌───────┐  │   │
│  │   │ Client 1 │  │ Client 2 │  │  ...  │  │   │
│  │   └────┬─────┘  └────┬─────┘  └───┬───┘  │   │
│  │        │              │            │      │   │
│  │        └──────────────┼────────────┘      │   │
│  │                       ▼                   │   │
│  │            Rc<RefCell<Store>>              │   │
│  │          (zero-cost shared state)         │   │
│  └───────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
```

- **Runtime**: `tokio::main(flavor = "current_thread")` — one OS thread, fully async.
- **Task spawning**: `tokio::task::spawn_local` — all tasks run on the same thread.
- **Shared state**: `Rc<RefCell<Store>>` — no `Arc`, no `Mutex`, zero lock contention by design.

This architecture guarantees **data-race freedom at compile time** while delivering excellent throughput for I/O-bound workloads.

---

## 📦 Module Breakdown

The codebase is organized into six focused modules:

```
src/
├── main.rs        → Entry point & runtime bootstrap
├── server.rs      → TCP listener & connection handler
├── resp.rs        → RESP protocol parser
├── cmd.rs         → Command routing & argument extraction
├── store.rs       → In-memory key-value engine
└── response.rs    → RESP response formatters
```

### `main.rs` — Entry Point
Bootstraps the single-threaded Tokio runtime and declares all modules. Calls `Server::run().await` and logs any fatal errors to stderr.

### `server.rs` — TCP Server
The `Server` struct binds to `127.0.0.1:7379`, creates a shared store (`Rc<RefCell<Store>>`), and accepts incoming TCP connections inside a `LocalSet`. Each connection is spawned via `spawn_local` and runs a loop that:
1. Reads raw bytes from the TCP stream into a `BytesMut` buffer
2. Decodes bytes into `RespValue` via `Resp::decode()`
3. Constructs a `RadishCommand` from the parsed value
4. Evaluates the command via `Response::eval()` against the shared store
5. Writes the RESP-encoded response bytes back to the client

### `resp.rs` — RESP Codec
Contains the `RespValue` enum and the `Resp` struct that provides both encoding and decoding:
- **`RespValue`**: `SimpleString`, `BulkString`, `Integer`, `Array`, `Error`, `Null`
- **`Resp::decode()`**: Recursive parser that handles all RESP type prefixes (`+`, `$`, `*`, `:`, `-`) and returns the decoded value plus remaining unconsumed buffer
- **`Resp::encode()`**: Serializes a `RespValue` back to wire format
- Convenience methods: `encode_simple_string()`, `encode_bulk_string()`, `encode_error()`, `encode_null()`

### `cmd.rs` — Command Router
Defines `CommandType` (`Ping`, `Echo`, `Set`, `Get`, `Ttl`, `Unknown`) with case-insensitive matching via `From<&str>`. The `RadishCommand` struct:
- Parses raw bytes through `from_bytes()` → `Resp::decode()` → `from_resp_value()`
- Extracts command name and arguments from `RespValue::Array`
- Exposes `cmd_type()` and `args()` getters

### `store.rs` — Key-Value Engine
A `HashMap<String, StoreValue>`-backed store where each value tracks:
- `value: RespValue` — the stored data (native RESP values)
- `expiry: Option<DateTime<Utc>>` — optional expiration timestamp

Exposes a `SharedStore` type alias (`Rc<RefCell<Store>>`) and a `Store::new()` factory. **Lazy expiration** is implemented in `get()` and `ttl()`: if a key's expiry has passed, it is treated as non-existent. No background scanning, no timers.

### `response.rs` — Command Evaluator
The `Response` struct holds raw RESP-encoded bytes (`Vec<u8>`) and provides `Response::eval()` — the business logic layer that:
- Pattern-matches on `CommandType` to dispatch each command
- Handles `SET` with both `EX` (seconds) and `PX` (milliseconds) options
- Borrows the store immutably for reads (`get`, `ttl`) and mutably for writes (`set`)
- Returns formatted RESP responses via the `Resp` encoding utilities

---

## 🔄 Request Lifecycle

Here's how a `SET mykey hello EX 60` command flows through Radish:

```
redis-cli                          Radish Server
    │                                    │
    │──── *5\r\n$3\r\nSET\r\n... ──────▶│
    │                                    │
    │                          ┌─────────┴──────────┐
    │                          │  resp.rs            │
    │                          │  Resp::decode()     │
    │                          │  → RespValue::Array │
    │                          └─────────┬──────────┘
    │                                    │
    │                          ┌─────────┴──────────┐
    │                          │  cmd.rs             │
    │                          │  RadishCommand::    │
    │                          │  from_resp_value()  │
    │                          │  → Set + args       │
    │                          └─────────┬──────────┘
    │                                    │
    │                          ┌─────────┴──────────┐
    │                          │  store.rs           │
    │                          │  Insert into        │
    │                          │  HashMap with       │
    │                          │  expiry timestamp   │
    │                          └─────────┬──────────┘
    │                                    │
    │                          ┌─────────┴──────────┐
    │                          │  response.rs        │
    │                          │  Response::eval()   │
    │                          │  → +OK\r\n          │
    │                          └─────────┬──────────┘
    │                                    │
    │◀─────────── +OK\r\n ───────────────│
```

---

## 🛠️ Tech Stack

| Component | Technology |
|-----------|-----------|
| Language | Rust (Edition 2024) |
| Async Runtime | [Tokio](https://tokio.rs/) (single-threaded flavor) |
| Buffer Management | [bytes](https://docs.rs/bytes/) (`BytesMut`) |
| Time & Expiry | [chrono](https://docs.rs/chrono/) (`DateTime<Utc>`) |
| Protocol | RESP (REdis Serialization Protocol) |

---

## 🚀 Getting Started

### Prerequisites
- [Rust](https://rustup.rs/) (edition 2024)
- `redis-cli` (optional, for interactive testing)

### Run the Server

```bash
# Clone the repository
git clone https://github.com/Ansh934/radish.git
cd radish

# Boot the server on 127.0.0.1:7379
cargo run
```

### Connect with redis-cli

```bash
# In another terminal
redis-cli -p 7379
```

### Example Session

```
127.0.0.1:7379> PING
PONG

127.0.0.1:7379> SET greeting "Hello, Radish!" EX 120
OK

127.0.0.1:7379> GET greeting
"Hello, Radish!"

127.0.0.1:7379> TTL greeting
(integer) 118

127.0.0.1:7379> ECHO "Radish is alive!"
"Radish is alive!"
```

---

## 📄 License

This project is open source. See the repository for license details.