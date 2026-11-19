<?php
// URL rewrite: /user/42 -> /profile.php?id=42
echo 'User ' . htmlspecialchars($_GET['id']);
