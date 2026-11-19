<?php
// Web example: configuration, embedded CSS and opt-in persistent uploads.
echo '<!doctype html><html lang="en"><head><meta charset="utf-8">';
echo '<meta name="viewport" content="width=device-width,initial-scale=1">';
echo '<title>Phix Web Example</title>';
echo '<link rel="stylesheet" href="/assets/site.css"></head><body><main>';
echo '<h1>', htmlspecialchars($_PHIX['website']['name']), '</h1>';
echo '<p>Built with Phix. Listening on ', $_PHIX['runtime']['bind'], ':', $_PHIX['runtime']['port'], '</p>';
if ($_FILES['photo']['tmp_name']) {
    $saved = move_uploaded_file($_FILES['photo']['tmp_name'], 'photo-upload.bin');
    if ($saved) {
        echo '<p class="ok">Upload saved to persistent storage.</p>';
    }
    if (!$saved) {
        echo '<p class="error">Upload not saved (file exists or was rejected).</p>';
    }
}
echo '<h2>Upload a file</h2>';
echo '<form method="post" action="/index.php" enctype="multipart/form-data">';
echo '<label for="photo">Choose a file (maximum 10 MiB)</label>';
echo '<input id="photo" type="file" name="photo" required>';
echo '<button type="submit">Upload</button></form>';
echo '<p class="note">This demo uses a fixed filename (photo-upload.bin): uploading another file does not overwrite the first. Files are private and cannot be downloaded directly by URL.</p>';
echo '</main></body></html>';
