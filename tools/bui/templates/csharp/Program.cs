using Bui;

var root = new Node("column");
root.Set("padding", 12);
root.Set("spacing", 6);

root.Add(new Label("{{title}}"));
var shown = new Label("Clicks: 0");
root.Add(shown);

var clicks = 0;
var button = new Button("Add one");
button.Click += (_, _) => shown.Text = $"Clicks: {++clicks}";
root.Add(button);

using var context = new Context(root);
using var window = new Window(context, "{{title}}", 320, 200);
window.Show();
Window.Run();
