# https://dev.to/deciduously/prepare-your-rust-api-docs-for-github-pages-2n5i
cargo doc --no-deps
rm -rf ./docs target/doc/
echo "<meta http-equiv=\"refresh\" content=\"0; url=build_wheel\">" > target/doc/index.html
cp -r target/doc ./docs
