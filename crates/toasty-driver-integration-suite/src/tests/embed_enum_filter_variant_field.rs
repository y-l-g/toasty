use crate::prelude::*;

/// Variant+field filter combined with a partition key so DynamoDB can execute it.
#[driver_test]
pub async fn filter_variant_field_with_partition_key(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum ContactInfo {
        #[column(variant = 1)]
        Email { address: String },
        #[column(variant = 2)]
        Phone { number: String },
    }

    #[derive(Debug, toasty::Model)]
    #[key(partition = group, local = id)]
    #[allow(dead_code)]
    struct User {
        #[auto]
        id: uuid::Uuid,
        group: String,
        name: String,
        contact: ContactInfo,
    }

    let mut db = t.setup_db(models!(User)).await;

    User::create()
        .group("eng")
        .name("Alice")
        .contact(ContactInfo::Email {
            address: "alice@example.com".to_string(),
        })
        .exec(&mut db)
        .await?;

    User::create()
        .group("eng")
        .name("Bob")
        .contact(ContactInfo::Phone {
            number: "555-1234".to_string(),
        })
        .exec(&mut db)
        .await?;

    User::create()
        .group("eng")
        .name("Carol")
        .contact(ContactInfo::Email {
            address: "carol@example.com".to_string(),
        })
        .exec(&mut db)
        .await?;

    // Partition key + variant field filter
    let results = User::filter(
        User::fields().group().eq("eng").and(
            User::fields()
                .contact()
                .email()
                .matches(|e| e.address().eq("alice@example.com")),
        ),
    )
    .exec(&mut db)
    .await?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Alice");

    Ok(())
}

/// OR of two variant-rooted field predicates — one per variant. Each predicate
/// on its own works; only the `OR` used to panic in the SQL serializer with
/// "unexpected enum discriminant" (issue #1061).
#[driver_test(requires(scan))]
pub async fn cross_variant_or(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Contact {
        #[column(variant = 1)]
        Email { address: String },
        #[column(variant = 2)]
        Phone { number: String },
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,
        contact: Contact,
    }

    let mut db = t.setup_db(models!(User)).await;

    User::create()
        .contact(Contact::Email {
            address: "x".to_string(),
        })
        .exec(&mut db)
        .await?;
    User::create()
        .contact(Contact::Phone {
            number: "x".to_string(),
        })
        .exec(&mut db)
        .await?;

    let rows = User::filter(
        User::fields()
            .contact()
            .email()
            .address()
            .eq("x")
            .or(User::fields().contact().phone().number().eq("x")),
    )
    .exec(&mut db)
    .await?;

    assert_eq!(rows.len(), 2);
    Ok(())
}

/// A variant field whose model type differs from its stored column type
/// (`uuid::Uuid`, stored as a string) is filterable. The decode cast the
/// enum's `Match` arm wraps around the column used to reach the SQL
/// serializer unchanged and panic; simplify now moves the conversion onto
/// the constant side.
#[driver_test]
pub async fn filter_variant_field_with_cast_storage(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Owner {
        #[column(variant = 1)]
        Human { id: uuid::Uuid },
        #[column(variant = 2)]
        Animal { tag: uuid::Uuid },
    }

    #[derive(Debug, toasty::Model)]
    struct Object {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Owner,
    }

    let mut db = t.setup_db(models!(Object)).await;

    let target = uuid::Uuid::new_v4();
    let expected = toasty::create!(Object {
        owner: Owner::Human { id: target }
    })
    .exec(&mut db)
    .await?;
    // Same UUID in the other variant's column: must not match.
    toasty::create!(Object {
        owner: Owner::Animal { tag: target }
    })
    .exec(&mut db)
    .await?;

    let found = Object::filter(
        Object::fields()
            .owner()
            .human()
            .matches(|h| h.id().eq(target)),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, expected.id);

    Ok(())
}

/// Same as above, but the variant field overrides its storage type away from
/// the backend default (`#[column(type = text)]` UUID; SQLite's default UUID
/// storage is a blob). The comparison constant must be converted to the
/// referenced column's stored type, not the backend default for the model
/// type — the latter silently matches zero rows.
#[driver_test(requires(sql))]
pub async fn filter_variant_field_with_storage_override(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Owner {
        #[column(variant = 1)]
        Human {
            #[column(type = text)]
            id: uuid::Uuid,
        },
        #[column(variant = 2)]
        Animal { tag: uuid::Uuid },
    }

    #[derive(Debug, toasty::Model)]
    struct Object {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Owner,
    }

    let mut db = t.setup_db(models!(Object)).await;

    let target = uuid::Uuid::new_v4();
    let expected = toasty::create!(Object {
        owner: Owner::Human { id: target }
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Object {
        owner: Owner::Human {
            id: uuid::Uuid::new_v4()
        }
    })
    .exec(&mut db)
    .await?;

    let found = Object::filter(
        Object::fields()
            .owner()
            .human()
            .matches(|h| h.id().eq(target)),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, expected.id);

    Ok(())
}

/// Filter the second field of a data variant. Its record slot (`local + 1` = 2)
/// exceeds the enum's variant count, so the slot must not be read as a schema
/// path step. `X::a` holding the filtered string must not match: the filter
/// selects the second field, not the variant. Both variants carry data;
/// filtering mixed unit+data enums is out of scope here (see #1058).
#[driver_test]
pub async fn filter_later_field_of_data_variant(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Contact {
        #[column(variant = 1)]
        X { a: String, b: String },
        #[column(variant = 2)]
        Y { c: String },
    }

    #[derive(Debug, toasty::Model)]
    #[key(partition = group, local = id)]
    #[allow(dead_code)]
    struct User {
        #[auto]
        id: uuid::Uuid,
        group: String,
        name: String,
        contact: Contact,
    }

    let mut db = t.setup_db(models!(User)).await;

    for (name, contact) in [
        (
            "Alice",
            Contact::X {
                a: "first".to_string(),
                b: "second".to_string(),
            },
        ),
        (
            "Bob",
            Contact::X {
                a: "second".to_string(),
                b: "other".to_string(),
            },
        ),
        (
            "Carol",
            Contact::Y {
                c: "second".to_string(),
            },
        ),
    ] {
        toasty::create!(User {
            group: "eng",
            name,
            contact,
        })
        .exec(&mut db)
        .await?;
    }

    let rows = User::filter(
        User::fields()
            .group()
            .eq("eng")
            .and(User::fields().contact().x().matches(|x| x.b().eq("second"))),
    )
    .exec(&mut db)
    .await?;

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "Alice");

    Ok(())
}
