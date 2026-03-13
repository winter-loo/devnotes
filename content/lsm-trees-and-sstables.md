---
title: "LSM Trees and SSTables: The Engine of Modern Databases"
date: "2026-03-13"
tags: ["database", "storage", "LSM", "SSTable"]
---

# LSM Trees and SSTables: The Engine of Modern Databases

Modern data-intensive applications demand high write throughput, low latency, and massive storage capacity. Traditional B-Tree based storage engines struggle to keep up with write-heavy workloads due to random disk I/O. This is where **Log-Structured Merge-Trees (LSM Trees)** and **Sorted String Tables (SSTables)** come in. They form the backbone of modern NoSQL databases like Cassandra, RocksDB, LevelDB, and DynamoDB.

## The Problem with B-Trees

B-Trees update data in place. When a write occurs, the database finds the corresponding page on disk and overwrites it. If a page fills up, it splits, leading to a cascading series of random disk writes. While HDDs and SSDs can handle sequential writes incredibly fast, random writes are orders of magnitude slower. In a high-throughput write scenario, B-Trees become an I/O bottleneck.

## Enter the LSM Tree

An LSM Tree flips this paradigm. Instead of updating data in place, it strictly appends data sequentially. Sequential I/O is blazing fast, allowing LSM Trees to achieve massive write throughput.

The core architecture of an LSM Tree consists of two main components:
1. **MemTable (In-Memory)**
2. **SSTables (On-Disk)**

### Step 1: The MemTable

When a write request arrives, it is first appended to a Write-Ahead Log (WAL) on disk for durability (crash recovery). Next, the data is inserted into an in-memory data structure called a **MemTable**. Typically implemented as a Red-Black Tree, Skip List, or AVL Tree, the MemTable keeps the incoming keys sorted.

### Step 2: Flushing to SSTables

Once the MemTable reaches a certain size threshold (e.g., a few megabytes), it is frozen and converted into an immutable **Sorted String Table (SSTable)**, which is then flushed to disk. A new MemTable is created to handle subsequent writes.

### Step 3: SSTables

An SSTable is a file format that stores a sequence of key-value pairs sorted by key. Because keys are sorted, finding a specific key is efficient. You don't need to scan the entire file; you can use an index (often sparse) kept in memory to jump to the relevant block in the SSTable, and then perform a quick binary search.

Since SSTables are immutable, updates and deletes are handled differently:
- **Updates:** Appended as a new key-value pair. The most recent version shadows older versions.
- **Deletes:** Appended as a "tombstone" marker, indicating the key has been deleted.

### Step 4: Compaction

As more MemTables are flushed, the number of SSTables grows. To read a key, the database might have to search the MemTable and then check multiple SSTables from newest to oldest. To prevent reads from becoming too slow and to reclaim disk space from overwritten/deleted keys, the database runs a background process called **Compaction**.

Compaction takes multiple smaller SSTables, merges them (similar to the merge step in merge sort), discards obsolete values and tombstones, and writes out a new, larger SSTable. This keeps the number of files manageable and maintains read performance.

## Conclusion

LSM Trees trade read performance and CPU (for compaction) in favor of extraordinary write performance (via sequential I/O). By understanding how MemTables buffer writes, SSTables store sorted data, and Compaction cleans up the mess, we can appreciate the elegant engineering behind the databases that power today's largest applications.
