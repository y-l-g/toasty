# Unique-violation error predicate

## Summary

Callers can detect duplicate-key failures with `Error::is_unique_violation()` instead of string-matching driver messages. SQL drivers map their duplicate-key codes to the new `UniqueViolation` variant; anything unrecognized stays `DriverOperationFailed`.

## Motivation

A model with `#[unique]` email rejects a duplicate insert, but the caller sees a generic `DriverOperationFailed` carrying the backend message. Turning that into an inline field error ("this email is taken") requires matching message text, which varies by backend, locale, and version. The existing typed variants cover `RecordNotFound` and `ConditionFailed` but nothing for uniqueness.

## User-facing API

### Detecting a duplicate

Call `is_unique_violation()` on the returned error. It returns true for single-field `#[unique]` conflicts, composite unique-index conflicts, and primary-key conflicts. It says only that a uniqueness rule fired, not which fields conflicted.

```rust
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,

    #[unique]
    email: String,
}

let err = User::create()
    .email("a@example.com")
    .exec(&mut db)
    .await
    .unwrap_err();

if err.is_unique_violation() {
    // render an inline "email taken" error
}
```

A composite unique index reports the same way. The check covers the whole index: the same slug under a different org succeeds, while the same `(org_id, slug)` pair fails.

```rust
#[unique(org_id, slug)]
struct Account {
    #[key]
    #[auto]
    id: u64,
    org_id: i64,
    slug: String,
}

let err = toasty::create!(Account { org_id: 1_i64, slug: "abc" })
    .exec(&mut db)
    .await
    .unwrap_err();

if err.is_unique_violation() {
    // duplicate (org_id, slug) pair
}
```

### Before and after

Before, callers matched backend text:

```rust
let msg = err.to_string();
if msg.contains("UNIQUE constraint failed") || msg.contains("Duplicate entry") {
    // fragile across backends and versions
}
```

After, callers match the predicate:

```rust
if err.is_unique_violation() {
    // works on SQLite, PostgreSQL, and MySQL
}
```

Cross-backend callers also check `is_condition_failed()` for DynamoDB updates, which report uniqueness-via-condition that way. DynamoDB inserts with a unique index currently stay `DriverOperationFailed`. The two predicates are not unified.

## Behavior

A duplicate single-field `#[unique]` value, a duplicate composite-index combination, or a duplicate primary key returns an error for which `is_unique_violation()` is true and `is_driver_operation_failed()` is false. The error display is `unique violation: <backend message>`, where the message is the backend's own text (for example `db_err.message()` on PostgreSQL).

Unrecognized codes stay `DriverOperationFailed`, so there are no false positives: an error that is not a duplicate-key failure never reports as one.

The predicate checks the outermost error kind, like every other `is_*` predicate. Wrapping an error with `.context()` hides the inner kind; the engine does not wrap driver errors this way on the execution path.

## Edge cases

- Composite unique conflicts report the whole index, with no per-field attribution.
- `NULL` values never produce a violation, per SQL semantics.
- Primary-key conflicts report as unique violations because backends use the same channel (SQLite `PRIMARYKEY` extended code, MySQL `ER_DUP_ENTRY`, PostgreSQL `23505` for both cases).
- On DynamoDB, update-path uniqueness failures surface as `ConditionFailed`, not `UniqueViolation`. Inserts with a unique index currently stay `DriverOperationFailed`.

## Driver integration

SQL drivers classify duplicate-key failures in their existing `classify_*` function, on the typed backend error before boxing it as `DriverOperationFailed`:

- SQLite: `SQLITE_CONSTRAINT_UNIQUE` and `SQLITE_CONSTRAINT_PRIMARYKEY` extended codes. Both are required; `UNIQUE` alone misses primary-key conflicts.
- PostgreSQL: SQLSTATE `23505`.
- MySQL: errors `1022`, `1062`, `1169`, `1586`, and `1859` (the set sqlx itself classifies as a unique violation).

No new capability flag and no new `Operation` variant. Out-of-tree drivers need no change: leaving duplicates as `DriverOperationFailed` remains valid, it just opts out of the predicate.

Turso leaves duplicates as `DriverOperationFailed`. Its `Error::Constraint` variant covers foreign-key violations as well as unique violations with no extended code to tell them apart, so mapping it wholesale would turn foreign-key failures into false unique positives. DynamoDB is unchanged: conditional-write failures stay `ConditionFailed`.

## Alternatives considered

**String-match driver messages.** Rejected: message text varies by backend, locale, and version. This is also why Turso stays out rather than sniffing for `"UNIQUE constraint failed"` inside `Error::Constraint`.

**Engine pre-check (SELECT before INSERT).** Rejected: the check races with concurrent writers. The database constraint is the source of truth.

**Constraint-name payload plus accessor.** Rejected: a constraint name without schema lookup does not identify the conflicting fields, and no error kind provides an accessor. Field attribution waits until a caller consumes it.

## Open questions

None. The predicate shape, the per-backend code sets, and the Turso and DynamoDB exclusions are settled.

## Out of scope

- Per-field or constraint-name attribution. Needs schema plus constraint lookup; SQLite does not expose it outside message text.
- Turso unique mapping. Blocked on upstream distinguishing unique from foreign-key failures.
- DynamoDB unification. `ConditionFailed` stays as the DynamoDB signal.
