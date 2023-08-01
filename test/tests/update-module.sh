#!/bin/bash

if [ "$DESCRIBE_TEST" = 1 ] ; then
	echo "This tests publishing a module without the --clear-database option"
        exit
fi

set -euox pipefail

source "./test/lib.include"

create_project

cat > "${PROJECT_PATH}/src/lib.rs" << EOF
use spacetimedb::{println, spacetimedb};

#[spacetimedb(table)]
pub struct Person {
    name: String,
}

#[spacetimedb(reducer)]
pub fn add(name: String) {
    Person::insert(Person { name });
}

#[spacetimedb(reducer)]
pub fn say_hello() {
    for person in Person::iter() {
        println!("Hello, {}!", person.name);
    }
    println!("Hello, World!");
}
EOF

IDENT=$(basename "$PROJECT_PATH")
run_test cargo run publish -s -d --project-path "$PROJECT_PATH" "$IDENT"
[ "1" == "$(grep -c "reated new database" "$TEST_OUT")" ]

run_test cargo run call "$IDENT" add '["Robert"]'
run_test cargo run call "$IDENT" add '["Julie"]'
run_test cargo run call "$IDENT" add '["Samantha"]'
run_test cargo run call "$IDENT" say_hello
run_test cargo run logs "$IDENT" 100
[ ' Hello, Samantha!' == "$(grep 'Samantha' "$TEST_OUT" | tail -n 4 | cut -d: -f4-)" ]
[ ' Hello, Julie!' == "$(grep 'Julie' "$TEST_OUT" | tail -n 4 | cut -d: -f4-)" ]
[ ' Hello, Robert!' == "$(grep 'Robert' "$TEST_OUT" | tail -n 4 | cut -d: -f4-)" ]
[ ' Hello, World!' == "$(grep 'World' "$TEST_OUT" | tail -n 4 | cut -d: -f4-)" ]

# Unchanged module is ok
run_test cargo run publish -s -d --project-path "$PROJECT_PATH" "$IDENT"
[ "1" == "$(grep -c "Updated database" "$TEST_OUT")" ]

# Changing an existing table isn't
cat > "${PROJECT_PATH}/src/lib.rs" <<EOF
use spacetimedb::spacetimedb;

#[spacetimedb(table)]
pub struct Person {
    name: String,
    age: u8,
}
EOF

run_test cargo run publish -s -d --project-path "$PROJECT_PATH" "$IDENT" || true
[ "1" == "$(grep -c "Error: Database update rejected" "$TEST_OUT")" ]

# Adding a table is ok, and invokes update
cat > "${PROJECT_PATH}/src/lib.rs" <<EOF
use spacetimedb::{println, spacetimedb};

#[spacetimedb(table)]
pub struct Person {
    name: String,
}

#[spacetimedb(table)]
pub struct Pet {
    species: String,
}

#[spacetimedb(update)]
pub fn on_module_update() {
    println!("MODULE UPDATED");
}
EOF

run_test cargo run publish -s -d --project-path "$PROJECT_PATH" "$IDENT"
[ "1" == "$(grep -c "Updated database" "$TEST_OUT")" ]
run_test cargo run logs "$IDENT" 1
[ ' MODULE UPDATED' == "$(grep 'MODULE UPDATED' "$TEST_OUT" | tail -n 1 | cut -d: -f4-)" ]
