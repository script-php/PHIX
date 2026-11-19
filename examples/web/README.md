# Phix web example — HTTP, config and persistent uploads

Build from the `phix` directory:

```sh
cargo build --release
mkdir -p /tmp/phix-web-deploy
./target/release/phix build examples/web -o /tmp/phix-web-deploy/app
/tmp/phix-web-deploy/app
```

Open http://127.0.0.1:9510/. The upload form submits to `index.php` using
`multipart/form-data`. Uploaded data is written **only** to the external directory
`/tmp/phix-web-deploy/storage/uploads`, configured by `storage.uploads` in
`phix.json`. The website source, assets and routing are compiled into the binary.

The demo uses the fixed destination `photo-upload.bin` because the current Phix
language prototype cannot yet safely derive unique filenames. The second upload
fails rather than overwriting existing data. Do not use the example as a public
upload service: implement authentication, content validation and generated
filenames before production use.

`config.json` beside the executable controls `runtime.port`, `runtime.bind`, and
`website.name`; it is copied on the first build and never overwritten on rebuild.
Existing upload files are also preserved on rebuild. The upload directory is
not published as static content. Its initial README file is copied only on first
deployment. Removing `storage.uploads` disables uploads.

Run `bash tests/web_example_upload.sh` from `phix/` for an end-to-end test.
