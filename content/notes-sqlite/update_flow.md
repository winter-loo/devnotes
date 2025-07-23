---
id: update_flow
aliases: []
tags: []
---


src/update.c

```
/*
** Process an UPDATE statement.
**
**   UPDATE OR IGNORE tbl SET a=b, c=d FROM tbl2... WHERE e<5 AND f NOT NULL;
**          \_______/ \_/     \______/      \_____/       \________________/
**           onError   |      pChanges         |                pWhere
**                     \_______________________/
**                               pTabList
*/
```

Reads:

- [Architecture of SQLite](https://www.sqlite.org/arch.html)
- [The SQLite Bytecode Engine](https://www.sqlite.org/opcode.html)
- [Atomic Commit In SQLite](https://www.sqlite.org/atomiccommit.html)
- [Query Planning](https://www.sqlite.org/queryplanner.html)

> [!TIP]
> - use [ast-grep](https://github.com/winter-loo/ast-grep-rules) tool to search codebase if LSP is not working


## FACTS

- `sqlite3_value` data structure is used as **registers** store.


## virtual machine

vdbe(Virtual Database Engine) trace

```bash
./sqlite3
> create table t(id int primary key, a int, b char(10));
> insert into t values (1, 2, 'hello');
> PRAGAMA vdbe_trace = 1;
> update t set a = 3 where id = 1;
```


instruction sequence:

```
┌─────┬───────────────┬────┬─────┬────┬──────────────────────────────┬────┬───────────────────────────────┐
│addr │ opcode        │ p1 │  p2 │ p3 │ p4                           │ p5 │  comment                      │
├─────┼───────────────┼────┼─────┼────┼──────────────────────────────┼────┼───────────────────────────────┤
│   0 │ Init          │  0 │ 18  │ 0  │                              │ 00 │ Start at 18                   │
│  18 │ Transaction   │  0 │  1  │ 1  │ 0                            │ 01 │ usesStmtJournal=0             │
│  19 │ TableLock     │  0 │  2  │ 1  │ t                            │ 00 │ iDb=0 root=2 write=1          │
│  20 │ Goto          │  0 │  1  │ 0  │                              │ 00 │                               │
│   1 │ Null          │  0 │  1  │ 2  │                              │ 00 │ r[1..2]=NULL                  │
│   2 │ Noop          │  2 │  0  │ 1  │                              │ 00 │                               │
│   3 │ OpenWrite     │  0 │  2  │ 0  │ 3                            │ 08 │ root=2 iDb=0; t               │
│   4 │ OpenWrite     │  1 │  3  │ 0  │ k(2,,)                       │ 02 │ root=3 iDb=0; sqlite_autoinde │
│   5 │ Explain       │  5 │  0  │ 0  │ SEARCH t                     │    │                               │
│     │               │    │     │    │ USING INDEX                  │    │                               │
│     │               │    │     │    │ sqlite_autoindex_t_1 (id= ?) │ 00 │                               │
│   6 │ Integer       │  1 │  6  │ 0  │                              │ 00 │ r[6]=1                        │
│   7 │ SeekGE        │  1 │ 11  │ 6  │ 1                            │ 00 │ key=r[6]                      │
│   9 │ DeferredSeek  │  1 │  0  │ 0  │                              │ 00 │ Move 0 to 1.rowid if needed   │
│  10 │ Rowid         │  0 │  2  │ 0  │                              │ 00 │ r[2]= rowid of 0              │
│  11 │ IsNull        │  2 │ 17  │ 0  │                              │ 00 │ if r[2]==NULL goto 17         │
│  12 │ Column        │  0 │  0  │ 3  │                              │ 00 │ r[3]= cursor 0 column 0       │
│  13 │ Integer       │  3 │  4  │ 0  │                              │ 00 │ r[4]=3                        │
│  14 │ Column        │  0 │  2  │ 5  │                              │ 00 │ r[5]= cursor 0 column 2       │
│  15 │ MakeRecord    │  3 │  3  │ 1  │ DDB                          │ 00 │ r[1]=mkrec(r[3..5])           │
│  16 │ Insert        │  0 │  1  │ 2  │ t                            │ 05 │ intkey=r[2] data=r[1]         │
│  17 │ Halt          │  0 │  0  │ 0  │                              │ 00 │                               │
└─────┴───────────────┴────┴─────┴────┴──────────────────────────────┴────┴───────────────────────────────┘
```

> [!TIP]
> - The P1, P2, and P3 operands are 32-bit signed integers. These operands often refer to registers.
> - P4 is a general purpose register and is used to store various kinds of data.
> - P5 is a 16-bit unsigned integer normally used to hold flags. A flag register.


Lookup **opcode** in `src/vdbe.c` for instruction description, say:

 - `case OP_Init`
 - `case OP_Transaction`
 - `case OP_Insert`
 - `case OP_Rowid`
 - `case OP_MakeRecord`


## ROWID
SQLite supports two types of tables:

1. Regular Tables (WITH ROWID) - Default
Have an implicit rowid: Every row has a unique 64-bit signed integer rowid
Automatically generated: If you don't specify a rowid, SQLite assigns one automatically
Can be accessed: You can reference it as rowid, oid, or _rowid_
Primary key behavior: If you declare an INTEGER PRIMARY KEY, it becomes an alias for the rowid
2. WITHOUT ROWID Tables
No implicit rowid: These tables do not have the automatic rowid column
Explicit declaration: Created by adding WITHOUT ROWID at the end of CREATE TABLE
Must have PRIMARY KEY: A PRIMARY KEY must be explicitly declared
Primary key is the key: The PRIMARY KEY serves as the record key instead of rowid



## virtual table

https://www.sqlite.org/vtab.html

```
/*
** Generate code for an UPDATE of a virtual table.
**
** There are two possible strategies - the default and the special
** "onepass" strategy. Onepass is only used if the virtual table
** implementation indicates that pWhere may match at most one row.
**
** The default strategy is to create an ephemeral table that contains
** for each row to be changed:
**
**   (A)  The original rowid of that row.
**   (B)  The revised rowid for the row.
**   (C)  The content of every column in the row.
**
** Then loop through the contents of this ephemeral table executing a
** VUpdate for each row. When finished, drop the ephemeral table.
**
** The "onepass" strategy does not use an ephemeral table. Instead, it
** stores the same values (A, B and C above) in a register array and
** makes a single invocation of VUpdate.
*/
static void updateVirtualTable;
```


An index is another table similar to the original "fruitsforsale" table but
with the content (the fruit column in this case) stored in front of the rowid
and with all rows in content order.


The general rule is that indexes are only useful if there are WHERE-clause
constraints on the left-most columns of the index.

## database file format

https://www.sqlite.org/fileformat2.html

see [src/btreeInt.h]

A database is a collection of btree pages.

[SQLite File Format Viewer](https://sqlite-internal.pages.dev/)
[An Exercise in Rust](https://github.com/winter-loo/snippets-rust/tree/main/codecrafters-sqlite)


### varint

A varint (variable-length integer) is a compact encoding scheme used by SQLite to store integers using a variable number of bytes, depending on the integer's value.

How Varint Works
Varint encoding uses:

1-7 bits per byte for the actual integer value
1 bit per byte as a continuation flag
Most significant bit (MSB) indicates if more bytes follow
Encoding Rules
Small integers (0-127): Use only 1 byte
Larger integers: Use multiple bytes, up to 9 bytes maximum
Each byte contributes 7 bits to the final value
MSB = 1: More bytes follow
MSB = 0: This is the last byte
