use crate::prelude::*;

#[driver_test(requires(sql), scenario(crate::scenarios::user_unique_email))]
pub async fn duplicate_unique_email_is_unique_violation(test: &mut Test) -> Result<()> {
    if test.capability().driver_name == "Turso" {
        // v1 leaves Turso on DriverOperationFailed; see
        // `assert_turso_stays_generic`.
        let mut db = setup(test).await;
        User::create().email("a@example.com").exec(&mut db).await?;
        let err = User::create()
            .email("a@example.com")
            .exec(&mut db)
            .await
            .unwrap_err();
        assert_turso_stays_generic(&err);
        return Ok(());
    }
    let mut db = setup(test).await;
    User::create().email("a@example.com").exec(&mut db).await?;
    let err = User::create()
        .email("a@example.com")
        .exec(&mut db)
        .await
        .unwrap_err();
    assert!(
        err.is_unique_violation(),
        "expected UniqueViolation, got: {err}"
    );
    assert!(
        !err.is_driver_operation_failed(),
        "expected not DriverOperationFailed, got: {err}"
    );
    // Update path: moving a second row onto a taken value reports the same way.
    let mut other = User::create().email("b@example.com").exec(&mut db).await?;
    let err = other
        .update()
        .email("a@example.com")
        .exec(&mut db)
        .await
        .unwrap_err();
    assert!(
        err.is_unique_violation(),
        "expected UniqueViolation, got: {err}"
    );
    assert!(
        !err.is_driver_operation_failed(),
        "expected not DriverOperationFailed, got: {err}"
    );
    // Negative: a non-duplicate failure is not a unique violation.
    let missing = User::get_by_email(&mut db, "missing@example.com")
        .await
        .unwrap_err();
    assert!(
        missing.is_record_not_found(),
        "expected RecordNotFound, got: {missing}"
    );
    assert!(
        !missing.is_unique_violation(),
        "expected not UniqueViolation, got: {missing}"
    );
    // Negative: a non-constraint driver failure is not a unique violation.
    // `missing_table_xyz` matches no real table, so every SQL driver
    // reports it as `DriverOperationFailed`.
    let bad_sql = toasty::sql::statement("SELECT * FROM missing_table_xyz")
        .exec(&mut db)
        .await
        .unwrap_err();
    assert!(
        bad_sql.is_driver_operation_failed(),
        "expected DriverOperationFailed, got: {bad_sql}"
    );
    assert!(
        !bad_sql.is_unique_violation(),
        "expected not UniqueViolation, got: {bad_sql}"
    );
    Ok(())
}

#[driver_test(requires(sql))]
pub async fn duplicate_primary_key_is_unique_violation(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: i64,
    }
    if t.capability().driver_name == "Turso" {
        // See `assert_turso_stays_generic`.
        let mut db = t.setup_db(models!(Item)).await;
        Item::create().id(1).exec(&mut db).await?;
        let err = Item::create().id(1).exec(&mut db).await.unwrap_err();
        assert_turso_stays_generic(&err);
        return Ok(());
    }
    let mut db = t.setup_db(models!(Item)).await;
    Item::create().id(1).exec(&mut db).await?;
    let err = Item::create().id(1).exec(&mut db).await.unwrap_err();
    assert!(
        err.is_unique_violation(),
        "expected UniqueViolation, got: {err}"
    );
    Ok(())
}

#[driver_test(requires(sql))]
pub async fn composite_unique_conflict_is_unique_violation(t: &mut Test) -> Result<()> {
    // Precedent: `composite_unique_index_enforced`
    // in `index_composite.rs` (local `#[unique(org_id, slug)]` model).
    #[derive(Debug, toasty::Model)]
    #[unique(org_id, slug)]
    struct Account {
        #[key]
        #[auto]
        id: u64,
        org_id: i64,
        slug: String,
    }
    if t.capability().driver_name == "Turso" {
        // See `assert_turso_stays_generic`.
        let mut db = t.setup_db(models!(Account)).await;
        toasty::create!(Account {
            org_id: 1_i64,
            slug: "abc"
        })
        .exec(&mut db)
        .await?;
        let err = toasty::create!(Account {
            org_id: 1_i64,
            slug: "abc"
        })
        .exec(&mut db)
        .await
        .unwrap_err();
        assert_turso_stays_generic(&err);
        return Ok(());
    }
    let mut db = t.setup_db(models!(Account)).await;
    toasty::create!(Account {
        org_id: 1_i64,
        slug: "abc"
    })
    .exec(&mut db)
    .await?;
    let err = toasty::create!(Account {
        org_id: 1_i64,
        slug: "abc"
    })
    .exec(&mut db)
    .await
    .unwrap_err();
    assert!(
        err.is_unique_violation(),
        "expected UniqueViolation, got: {err}"
    );
    // Same slug under a different org is allowed — proves whole-index match.
    toasty::create!(Account {
        org_id: 2_i64,
        slug: "abc"
    })
    .exec(&mut db)
    .await?;
    // Same org under a different slug is allowed — other direction.
    toasty::create!(Account {
        org_id: 1_i64,
        slug: "xyz"
    })
    .exec(&mut db)
    .await?;
    Ok(())
}

/// v1 leaves Turso on `DriverOperationFailed` (see Driver integration in
/// `docs/dev/design/unique-violation.md`): its `Error::Constraint` covers
/// foreign-key violations as well as unique violations with no extended
/// code to tell them apart. Assert the negative to lock v1 behavior so the
/// test fails loudly when upstream later distinguishes unique from FK.
fn assert_turso_stays_generic(err: &toasty::Error) {
    assert!(
        err.is_driver_operation_failed(),
        "expected DriverOperationFailed, got: {err}"
    );
    assert!(
        !err.is_unique_violation(),
        "Turso must not report UniqueViolation in v1, got: {err}"
    );
}
