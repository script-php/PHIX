# Phix upload storage — Milestone 18

A compiled Phix website has two kinds of files:

- **Embedded, immutable resources:** CSS, JavaScript, images, templates; stored in the executable.
- **Mutable upload storage:** optional directory on disk beside the executable; never embedded or served over HTTP.

Storage is **disabled by default**. To opt in, add this to the source project's `phix.json`:

```json
{
  "entry": "index.php",
  "storage": {
    "uploads": "storage/uploads",
    "max_file_size": 10485760
  }
}
```

Both paths are relative to the project at build time and to the executable's directory at runtime.
The upload directory may initially contain files. On first deployment, Phix creates the same directory beside the output executable and copies the initial files **without overwriting any existing files**. Subsequent builds preserve the whole directory; deleted files are not restored. The marker `.phix-storage-initialized` records first initialization. Never remove it in a deployed storage directory unless you specifically intend a new initialization.

When `storage.uploads` is absent, Phix creates no storage directory and rejects multipart upload requests. `storage.max_file_size` defaults to 10 MiB when enabled (maximum supported setting: 32 MiB per file). Up to 16 files may be supplied per request; total request size is limited. The current HTTP server is a development implementation, not a production-grade HTTP/multipart parser.

## Application-side upload handling

```php
<?php
if ($_FILES['photo']['tmp_name']) {
    if (move_uploaded_file($_FILES['photo']['tmp_name'], 'photo.png')) {
        echo 'Saved';
    }
}
```

`$_FILES['photo']` currently contains `name`, `size`, `type`, `error`, `tmp_name` for a single uploaded file. `tmp_name` is an opaque in-memory identifier, not a filesystem pathname. Upload bytes remain request-local until the app calls `move_uploaded_file()`. This function returns false when storage is disabled, the token does not exist, the destination is invalid, or the target already exists. Both `photo.png` and `storage/uploads/photo.png` are accepted as destinations in this example. All destinations are interpreted within the configured directory.

**Security restrictions**:

- No write is performed merely by receiving a multipart request: application code must explicitly call `move_uploaded_file()`.
- The destination is limited to a relative path *inside* the configured upload directory; absolute paths, `..`, backslashes, dotfiles, and symlink traversal are rejected.
- Existing files cannot be overwritten (`create_new`).
- Storage is omitted from the embedded asset bundle and is never served directly via the compiled HTTP static asset router, even if it lies within the document root.
- Symlinks in the source storage tree or destination folders are rejected during deployment.
- Uploaded files are not executable PHP entrypoints and are not exposed as public URLs.

**Not yet implemented:** PHP's complete `$_FILES` bracket-array semantics, multi-upload fields with the same field name, `$_POST` form values, `is_uploaded_file()`, upload error codes, arbitrary PHP filesystem access, and an authenticated file-download API. Your application is responsible for checking authorization and allowed file types before calling `move_uploaded_file()`. Phix is not yet suitable for public internet exposure.
