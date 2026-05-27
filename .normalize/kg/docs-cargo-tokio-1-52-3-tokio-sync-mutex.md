---
fetched_at: 2026-05-27T08:14:07.196983437+00:00
item_kind: struct
kind: docs
language: rust
package: tokio
source_url: https://docs.rs/tokio/1.52.3/tokio/sync/struct.Mutex.html
symbol_path: tokio::sync::Mutex
version: 1.52.3
links:
- kind: source
  to: https://docs.rs/tokio/1.52.3/tokio/sync/struct.Mutex.html
---
# tokio::Mutex (rust, tokio 1.52.3)

struct

```rust
pub struct Mutex<T: ?Sized> { /* private fields */ }
```

An asynchronous `Mutex`-like type.


This type acts similarly to `std::sync::Mutex`, with two major
differences: `lock` is an async method so does not block, and the lock
guard is designed to be held across `.await` points.


Tokio’s Mutex operates on a guaranteed FIFO basis.
This means that the order in which tasks call the `lock` method is
the exact order in which they will acquire the lock.


§Which kind of mutex should you use?

Contrary to popular belief, it is ok and often preferred to use the ordinary
`Mutex` from the standard library in asynchronous code.


The feature that the async mutex offers over the blocking mutex is the
ability to keep it locked across an `.await` point. This makes the async
mutex more expensive than the blocking mutex, so the blocking mutex should
be preferred in the cases where it can be used. The primary use case for the
async mutex is to provide shared mutable access to IO resources such as a
database connection. If the value behind the mutex is just data, it’s
usually appropriate to use a blocking mutex such as the one in the standard
library or `parking_lot`.


Note that, although the compiler will not prevent the std `Mutex` from holding
its guard across `.await` points in situations where the task is not movable
between threads, this virtually never leads to correct concurrent code in
practice as it can easily lead to deadlocks.


A common pattern is to wrap the `Arc<Mutex<...>>` in a struct that provides
non-async methods for performing operations on the data within, and only
lock the mutex inside these methods. The mini-redis example provides an
illustration of this pattern.


Additionally, when you *do* want shared access to an IO resource, it is
often better to spawn a task to manage the IO resource, and to use message
passing to communicate with that task.


§Examples:

`use tokio::sync::Mutex;
use std::sync::Arc;

let data1 = Arc::new(Mutex::new(0));
let data2 = Arc::clone(&data1);

tokio::spawn(async move {
    let mut lock = data2.lock().await;
    *lock += 1;
});

let mut lock = data1.lock().await;
*lock += 1;`
`use tokio::sync::Mutex;
use std::sync::Arc;

let count = Arc::new(Mutex::new(0));

for i in 0..5 {
    let my_count = Arc::clone(&count);
    tokio::spawn(async move {
        for j in 0..10 {
            let mut lock = my_count.lock().await;
            *lock += 1;
            println!("{} {} {}", i, j, lock);
        }
    });
}

loop {
    if *count.lock().await >= 50 {
        break;
    }
}
println!("Count hit 50.");`
There are a few things of note here to pay attention to in this example.


The mutex is wrapped in an `Arc` to allow it to be shared across
threads.
Each spawned task obtains a lock and releases it on every iteration.
Mutation of the data protected by the Mutex is done by de-referencing
the obtained lock as seen on lines 13 and 20.

Tokio’s Mutex works in a simple FIFO (first in, first out) style where all
calls to `lock` complete in the order they were performed. In that way the
Mutex is “fair” and predictable in how it distributes the locks to inner
data. Locks are released and reacquired after every iteration, so basically,
each thread goes to the back of the line after it increments the value once.
Note that there’s some unpredictability to the timing between when the
threads are started, but once they are going they alternate predictably.
Finally, since there is only a single valid lock at any given time, there is
no possibility of a race condition when mutating the inner value.


Note that in contrast to `std::sync::Mutex`, this implementation does not
poison the mutex when a thread holding the `MutexGuard` panics. In such a
case, the mutex will be unlocked. If the panic is caught, this might leave
the data protected by the mutex in an inconsistent state.

## Examples

```rust
use tokio::sync::Mutex;
use std::sync::Arc;

let data1 = Arc::new(Mutex::new(0));
let data2 = Arc::clone(&data1);

tokio::spawn(async move {
    let mut lock = data2.lock().await;
    *lock += 1;
});

let mut lock = data1.lock().await;
*lock += 1;
```

```rust
use tokio::sync::Mutex;
use std::sync::Arc;

let count = Arc::new(Mutex::new(0));

for i in 0..5 {
    let my_count = Arc::clone(&count);
    tokio::spawn(async move {
        for j in 0..10 {
            let mut lock = my_count.lock().await;
            *lock += 1;
            println!("{} {} {}", i, j, lock);
        }
    });
}

loop {
    if *count.lock().await >= 50 {
        break;
    }
}
println!("Count hit 50.");
```

Source: <https://docs.rs/tokio/1.52.3/tokio/sync/struct.Mutex.html>
