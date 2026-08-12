// Inert on purpose. The build pipeline (`toolchain::native_sources()`)
// only compiles .c/.cpp/.cc/.cxx, so no `.f90` branch exists yet. Wiring
// templates in before that's fixed would let scaffolding succeed and
// `carrier install --install-deps` fail on the very next step, with a
// worse error ("Makevars found but no `.c`/`.cpp`/`.cc`/`.cxx` sources") than
// scaffold.rs's own upfront bail! gives today. Fill this in once
// `native_sources()` gains an `.f90` arm, not before.