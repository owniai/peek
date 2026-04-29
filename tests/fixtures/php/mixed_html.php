<!DOCTYPE html>
<html>
<head><title>Test</title></head>
<body>
<?php echo "<h1>Hello</h1>"; ?>
<p>Some HTML</p>
<?php
class Page {
    public function render(): string {
        return "rendered";
    }
}
?>
</body>
</html>
