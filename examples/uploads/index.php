<?php
if ($_FILES['photo']['tmp_name']) {
    if (move_uploaded_file($_FILES['photo']['tmp_name'], 'photo.bin')) {
        echo 'Upload saved';
    }
}
echo '<form action="/index.php" method="post" enctype="multipart/form-data">';
echo '<input type="file" name="photo">';
echo '<button type="submit">Upload</button></form>';
