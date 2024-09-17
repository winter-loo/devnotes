use std::ptr::NonNull;

#[derive(Debug)]
struct Header {
    a: u64,
}

#[derive(Debug)]
struct Cell {
    a: Header,
    b: u64,
    c: u64,
}

impl Drop for Cell {
    fn drop(&mut self) {
        println!("Cell dropped");
    }
}

#[derive(Debug, Clone)]
struct RawTask {
    a: NonNull<Header>,
}

impl Copy for RawTask {}

#[derive(Debug)]
struct Task {
    raw: RawTask,
}

impl Drop for Task {
    fn drop(&mut self) {
        println!("task dropped");
    }
}

#[derive(Debug)]
struct Notified(Task);

fn main() {
    let cell = Box::new(Cell{
        a: Header { a: 1 },
        b: 2,
        c: 3,
    });

    let ptr = Box::into_raw(cell);
    let ptr = unsafe { NonNull::new_unchecked(ptr.cast()) };

    let raw = RawTask {
        a: ptr,
    };
    let task = Task {
        raw,
    };
    let notified = Notified(Task {
        raw,
    });

    // std::mem::forget(task);
    // std::mem::forget(notified);

    let cell2 = unsafe { Box::from_raw(raw.a.as_ptr() as *mut Cell) };
    let cell3 = unsafe { Box::from_raw(task.raw.a.as_ptr() as *mut Cell) };
    let cell4 = unsafe { Box::from_raw(notified.0.raw.a.as_ptr() as *mut Cell) };

    println!("{:#?}", cell2);
    println!("{:#?}", cell3);
    println!("{:#?}", cell4);
}
