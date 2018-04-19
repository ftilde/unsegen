#! /bin/sh
echo "-> fmt in root"
cargo fmt
for dir in unsegen-*; do
    cd $dir
    echo "-> fmt in $dir"
    cargo fmt
    cd - > /dev/null
done
