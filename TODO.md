# Known panics

 - Extended network dropouts:

    ```
    thread 'main' panicked at 'called `Result::unwrap()` on an `Err` value: Error(Io(Error { repr: Os { code: 101, message: "Network is unreachable" } })
    ```

 - Poorly formatted toml configuration
 - Various failures connecting to the IRC server, needs added exception handling.

# Extra features

 - More efficient use of curl (stop fetching page once title has been obtained)
 - Better handling of IRC connection errors (don't panic)
 - Set config file path on command line option / have default search paths, local last
 - Update to updated rust IRC API
 - Batch log processor to add historical logs to the database

# Tests

 - Previous postings
