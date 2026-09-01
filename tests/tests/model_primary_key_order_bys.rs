//! `Model::primary_key_order_bys` returns one ascending order-by per
//! primary-key field, in primary-key order — partition fields before local,
//! regardless of field declaration order.

use toasty::schema::Model;

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    id: i64,
    name: String,
}

#[derive(Debug, toasty::Model)]
struct Membership {
    #[key]
    org_id: i64,
    #[key]
    user_id: i64,
}

// `id` is declared first but is the *local* key: the primary-key order must
// put the partition field first.
#[derive(Debug, toasty::Model)]
#[key(partition = tenant, local = id)]
struct TenantRecord {
    id: i64,
    tenant: String,
}

#[test]
fn appends_pk_asc_after_user_order_by() {
    let mut order_bys = vec![User::fields().name().asc()];
    order_bys.extend(User::primary_key_order_bys());
    assert_eq!(
        order_bys,
        vec![User::fields().name().asc(), User::fields().id().asc()]
    );
}

#[test]
fn composite_pk_fields_in_declaration_order() {
    assert_eq!(
        Membership::primary_key_order_bys(),
        vec![
            Membership::fields().org_id().asc(),
            Membership::fields().user_id().asc(),
        ]
    );
}

#[test]
fn partition_fields_come_before_local() {
    assert_eq!(
        TenantRecord::primary_key_order_bys(),
        vec![
            TenantRecord::fields().tenant().asc(),
            TenantRecord::fields().id().asc(),
        ]
    );
}
