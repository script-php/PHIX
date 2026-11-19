<?php
// A framework-style front controller receives URLs not matched by public files.
echo '<h1>Phix front controller</h1>';
echo htmlspecialchars($_SERVER['REQUEST_URI']);
