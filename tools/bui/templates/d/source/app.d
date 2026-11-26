import bui;
import std.conv : to;

void main() {
    auto root = new Node("column");
    root.set("padding", 12);
    root.set("spacing", 6);

    auto title = new Node("label");
    title.set("text", "{{title}}");
    root.add(title);

    int count;
    auto shown = new Node("label");
    shown.set("text", "Clicks: 0");
    root.add(shown);

    auto button = new Node("button");
    button.set("text", "Add one");
    button.onClick((NodeEvent) {
        ++count;
        shown.set("text", "Clicks: " ~ count.to!string);
    });
    root.add(button);

    auto context = new Context(root);
    auto window = new Window(context, "{{title}}", 320, 200);
    window.show();
    run();

    window.free();
    context.free();
    root.free();
}
