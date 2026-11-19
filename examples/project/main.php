<?php
require_once "lib/math.phx";
echo "Result: ", double(21), "\n";
echo "Asset: ", asset_text("assets/message.txt"), "\n";
echo "Size: ", asset_size("assets/message.txt"), "\n";
if (asset_exists("assets/message.txt")) {
    echo "Asset embedded\n";
}
