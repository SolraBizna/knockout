use std::io;
use std::io::Write;

pub fn write_header<W: Write>(mut out: W) -> io::Result<()> {
    out.write_all(br###"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<title>Knockout Exclude Check output</title>
"###)?;
    if cfg!(feature = "external-code") {
        out.write_all(br#"<link rel="stylesheet" type="text/css" href="css.css">
"#)?;
    }
    else {
        out.write_all(br###"<style type="text/css"><!--
/* Meyer reset variant */
* {
    margin: 0;
    padding: 0;
    border: 0;
    font-size: inherit;
    font-family: inherit;
    vertical-align: baseline;
    font-weight: inherit;
    font-style: inherit;
    text-decoration: inherit;
    box-sizing: content-box;
}
body { line-height: 1.15; }
blockquote,q { quotes: none; }
blockquote:before, blockquote:after, q:before, q:after {
    content: '';
    content: none;
}
table { border-collapse: collapse; border-spacing: 0; }
/* Actual CSS */
body {
    background-color: #000;
    color: #fff;
    font-family: sans;
    font-size: 12px;
    margin: 1em 0;
}
pre.errors {
    color: #f77;
}
pre {
    margin: 0.5em 1em;
    font-family: mono;
    white-space: pre-wrap;
}

div div { margin-left: 10px; }

button {
    margin: 2px;
    width: 20px;
    height: 20px;
    vertical-align: baseline;
    border: 2px solid #fff;
    background-color: #000;
    color: #fff;
}
button:active, button:active.selected { background-color: #fff; color: #000; }
button.selected { background-color: #ccc; color: #000; }
button:disabled, button:active:disabled { background-color: #000; border-color: #666; color: #666; }
p.disabled { color: #666; }
button:disabled.selected, button:active:disabled.selected { background-color: #444; color: #000; }

.trivial { color: #7ff; }
.trivial button {
    border: 2px solid #7ff;
    color: #7ff;
}
.trivial button:active, p.trivial button:active.selected { background-color: #7ff; color: #000; }
.trivial button.selected { background-color: #6cc; color: #000; }
.trivial button:disabled, .trivial button:active:disabled { border-color: #366; color: #366; }
p.disabled.trivial { color: #488; }
.trivial button:disabled.selected, .trivial button:active:disabled.selected { background-color: #244; color: #000; }

.excluded { color: #f77; }
.excluded button {
    border: 2px solid #f77;
    color: #f77;
}
.excluded button:active, p.excluded button:active.selected { background-color: #f77; color: #000; }
.excluded button.selected { background-color: #c66; color: #000; }
.excluded button:disabled, .excluded button:active:disabled { border-color: #633; color: #633; }
p.disabled.excluded { color: #844; }
.excluded button:disabled.selected, .excluded button:active:disabled.selected { background-color: #422; color: #000; }

.vetted { color: #77f; }
.vetted button {
    border: 2px solid #77f;
    color: #77f;
}
.vetted button:active, p.vetted button:active.selected { background-color: #77f; color: #000; }
.vetted button.selected { background-color: #66c; color: #000; }
.vetted button:disabled, .vetted button:active:disabled { border-color: #336; color: #336; }
p.disabled.vetted { color: #448; }
.vetted button:disabled.selected, .vetted button:active:disabled.selected { background-color: #224; color: #000; }

hr { border: 2px solid #ccc; margin: 8px 0; }
--></style>
"###)?;
    }
    out.write_all(br###"</head>
<body>
<noscript>You must have JavaScript enabled in order to use this widget.</noscript>
<script><!--
"###)?;
    Ok(())
}

pub fn write_footer<W: Write>(mut out: W) -> io::Result<()> {
    out.write_all(br###"//--></script>
"###)?;
    if cfg!(feature = "external-code") {
        out.write_all(br#"<script src="js.js"></script>
"#)?;
    }
    else {
        out.write_all(br###"<script><!--
"use strict";

let tree = [];
{
    const EXCLUDE_ICON = "\u2717";
    const VET_ICON = "\u2713";
    const FILE_ICON = "\u25cc";
    const FOLDER_CLOSED_ICON = "\u25b6";
    const FOLDER_OPEN_ICON = "\u25bc";
    let vet_list = document.createElement("pre");
    let excl_list = document.createElement("pre");
    let calculate_triviality = function(el, do_not_recurse) {
        if(el.children == null) return;
        let trivial = true;
        for(let n = 0; n < el.children.length; ++n) {
            let child = el.children[n];
            if((child.type == "file" || child.type == "dir")
               && !child.vetted && !child.excluded && !child.trivial) {
                trivial = false;
                break;
            }
        }
        if(trivial === el.trivial) return;
        el.trivial = trivial;
        if(el.nodes) {
            if(trivial) el.nodes.p.classList.add("trivial");
            else el.nodes.p.classList.remove("trivial");
        }
        if(el.parent && !do_not_recurse) {
            calculate_triviality(el.parent, do_not_recurse);
        }
    };
    let compare_els_by_size = function(a,b) {
        if(a.size === undefined && b.size !== undefined) return 1;
        else if(a.size !== undefined && b.size === undefined) return -1;
        else if(a.size < b.size) return 1;
        else if(a.size > b.size) return -1;
        else if(a.name < b.name) return -1;
        else if(a.name > b.name) return 1;
        else return 0;
    };
    let recursively_build_list = function(el, vets_to_add, excludes_to_add) {
        if(el.vetted) {
            if(el.type == "dir") {
                vets_to_add.push(el.path+"***");
            }
            else {
                vets_to_add.push(el.path);
            }
        }
        else if(el.excluded) {
            excludes_to_add.push(el.path);
        }
        if(el.type == "dir" && !el.excluded) {
            for(let n = 0; n < el.children.length; ++n) {
                recursively_build_list(el.children[n], vets_to_add, excludes_to_add);
            }
        }
    };
    let rebuild_lists = function() {
        let vets_to_add = [];
        let excludes_to_add = [];
        for(let n = 0; n < tree.length; ++n) {
            recursively_build_list(tree[n], vets_to_add, excludes_to_add);
        }
        let text = [];
        if(vets_to_add.length == 0) {
            text.push("No new \"vetted\" entries.");
        }
        else {
            text.push("Add the following entries to \"vetted\":\n");
            for(let n = 0; n < vets_to_add.length; ++n) {
                text.push("/"+vets_to_add[n]);
            }
        }
        vet_list.innerText = text.join("\n");
        text = [];
        if(excludes_to_add.length == 0) {
            text.push("No new \"excludes\" entries.");
        }
        else {
            text.push("Add the following entries to \"excludes\":\n");
            for(let n = 0; n < excludes_to_add.length; ++n) {
                text.push("/"+excludes_to_add[n]);
            }
        }
        excl_list.innerText = text.join("\n");
    };
    let make_button = function(label, callback, enabled, selected) {
        let button = document.createElement("button");
        button.appendChild(document.createTextNode(label));
        if(!enabled) button.setAttribute("disabled", "disabled");
        if(selected) button.classList.add("selected");
        button.onclick = callback;
        return button;
    };
    let select_button = function(el, btn) {
        for(let n = 0; n < el.nodes.buttons.length; ++n) {
            if(n == btn) el.nodes.buttons[n].classList.add("selected");
            else el.nodes.buttons[n].classList.remove("selected");
        }
    };
    let alter_size = function(el) {
        // This can accumulate floating point error with a large number of uses
        // with a wide variety of magnitudes. But we already have enough O(n^2)
        // stuff in here, so having a count that's off by a few bytes isn't
        // that big a deal compared to keeping the performance acceptable.
        let should_subtract = el.vetted || el.excluded;
        if(should_subtract && !el.subtracted) {
            el.subtracted = true;
            let parent = el.parent;
            while(parent) {
                parent.size -= el.size;
                parent.nodes.size.innerText = get_size_for_display(parent.size);
                parent = parent.parent;
            }
        }
        else if(!should_subtract && el.subtracted) {
            el.subtracted = false;
            let parent = el.parent;
            while(parent) {
                parent.size += el.size;
                parent.nodes.size.innerText = get_size_for_display(parent.size);
                parent = parent.parent;
            }
        }
    };
    let neutralize = function(el) {
        if(el.vetted) {
            el.vetted = undefined;
            el.nodes.p.classList.remove("vetted");
        }
        if(el.excluded) {
            el.excluded = undefined;
            el.nodes.p.classList.remove("excluded");
        }
        if(el.disclosed) {
            el.disclosed = undefined;
            for(let n = 0; n < el.children.length; ++n) {
                let child = el.children[n];
                child.nodes.div.setAttribute("style", "display:none");
            }
        }
    };
    let exclude = function(el) {
        if(el.excluded) return;
        neutralize(el);
        el.excluded = true;
        select_button(el, 0);
        el.nodes.p.classList.add("excluded");
        rebuild_lists();
        if(el.parent) {
            calculate_triviality(el.parent);
            alter_size(el);
        }
    };
    let vet = function(el) {
        if(el.vetted) return;
        neutralize(el);
        el.vetted = true;
        select_button(el, 1);
        el.nodes.p.classList.add("vetted");
        rebuild_lists();
        if(el.parent) {
            calculate_triviality(el.parent);
            alter_size(el);
        }
    };
    let undecide = function(el) {
        neutralize(el);
        if(el.type == "dir" && el.children.length > 0) select_button(el, 2);
        else select_button(el, 3);
        rebuild_lists();
        if(el.parent) {
            calculate_triviality(el.parent);
            alter_size(el);
        }
    };
    let disclose = function(el) {
        console.assert(el.type == "dir");
        if(el.disclosed) return;
        neutralize(el);
        el.disclosed = true;
        select_button(el, 3);
        if(!el.ever_disclosed) {
            el.ever_disclosed = true;
            for(let n = 0; n < el.children.length; ++n) {
                let child = el.children[n];
                child.nodes = make_nodes(child);
                el.nodes.div.appendChild(child.nodes.div);
            }
        }
        else {
            for(let n = 0; n < el.children.length; ++n) {
                let child = el.children[n];
                child.nodes.div.removeAttribute("style");
            }
        }
        rebuild_lists();
        if(el.parent) {
            calculate_triviality(el.parent);
            alter_size(el);
        }
    };
    const DISPLAY_UNITS = [
        {"div":1000000000000, "suffix":" TB", precision:2},
        {"div":1000000000, "suffix":" GB", precision:2},
        {"div":1000000, "suffix":" MB", precision:2},
        {"div":1024, "suffix":" KiB", precision:0},
    ];
    let get_size_for_display = function(size) {
        for(let n = 0; n < DISPLAY_UNITS.length; ++n) {
            let unit = DISPLAY_UNITS[n];
            if(size >= unit.div)
                return (size / unit.div).toFixed(unit.precision) + unit.suffix;
        }
        return size + " bytes";
    };
    let make_nodes = function(el) {
        let nodes = {};
        let div = document.createElement("div");
        nodes.div = div;
        let p = document.createElement("p");
        nodes.p = p;
        div.appendChild(p);
        let buttons = [];
        nodes.buttons = buttons;
        switch(el.type) {
        case "excluded":
            p.classList.add("excluded");
            p.classList.add("disabled");
            buttons.push(make_button(EXCLUDE_ICON, null, false, true));
            buttons.push(make_button(VET_ICON, null, false, false));
            break;
        case "vetted":
            p.classList.add("vetted");
            p.classList.add("disabled");
            buttons.push(make_button(EXCLUDE_ICON, null, false, false));
            buttons.push(make_button(VET_ICON, null, false, true));
            break;
        case "file":
            buttons.push(make_button(EXCLUDE_ICON, function() { exclude(el) },
                                     true, false));
            buttons.push(make_button(VET_ICON, function() { vet(el) },
                                     true, false));
            buttons.push(make_button("\xA0", null, false, false));
            buttons.push(make_button(FILE_ICON, function() { undecide(el) },
                                     true, true));
            break;
        case "dir":
            buttons.push(make_button(EXCLUDE_ICON, function() { exclude(el) },
                                     true, false));
            buttons.push(make_button(VET_ICON, function() { vet(el) },
                                     true, false));
            if(el.children.length > 0) {
                buttons.push(make_button(FOLDER_CLOSED_ICON, function() { undecide(el) },
                                         true, true));
                buttons.push(make_button(FOLDER_OPEN_ICON, function() { disclose(el) },
                                         true, false));
            }
            else {
                buttons.push(make_button(FOLDER_CLOSED_ICON,
                                         false, false));
                buttons.push(make_button(FOLDER_OPEN_ICON, function() { undecide(el) },
                                         true, true));
            }
            break;
        case "error_dir":
            p.classList.add("error");
            p.classList.add("disabled");
            break;
        case "mount":
            p.classList.add("disabled");
            break;
        }
        if(el.trivial) p.classList.add("trivial");
        while(buttons.length < 4) {
            buttons.push(make_button("\xA0", null, false, false));
        }
        for(let n = 0; n < buttons.length; ++n) {
            p.appendChild(buttons[n]);
        }
        p.appendChild(document.createTextNode(" "+el.path));
        if(el.size !== undefined) {
            p.appendChild(document.createTextNode(" ("));
            let size = document.createElement("span");
            nodes.size = size;
            size.innerText = get_size_for_display(el.size);
            p.appendChild(size);
            p.appendChild(document.createTextNode(")"));
        }
        if(el.type == "error_dir") {
            p.appendChild(document.createTextNode(" (error)"));
        }
        else if(el.type == "mount") {
            p.appendChild(document.createTextNode(" (mount point)"));
        }
        return nodes;
    };
    let find_path = function(el) {
        let ret;
        if(el.parent != null) {
            console.assert(el.parent.path.endsWith("/"));
            ret = el.parent.path + el.name;
        }
        else {
            ret = el.name;
        }
        if(el.type == "dir" || el.type == "error_dir") {
            return ret + "/";
        }
        else {
            return ret;
        }
    };
    let convert = function(el, parent) {
        if(Array.isArray(el)) {
            console.assert(el[0].startsWith("d"));
            let ret = {type:"dir", name:el[0].substr(1), parent:parent, children:[]};
            ret.path = find_path(ret);
            for(let n = 1; n < el.length; ++n) {
                ret.children[n-1] = convert(el[n], ret);
            }
            ret.children.sort(compare_els_by_size);
            let size = 0;
            for(let n = ret.children.length-1; n >= 0; --n) {
                let child_size = ret.children[n].size;
                if(child_size !== undefined)
                    size += child_size;
            }
            ret.size = size;
            calculate_triviality(ret, true);
            return ret;
        }
        else {
            let name = el.substr(1);
            let ret = {name:name, parent:parent};
            switch(el[0]) {
            case "f": {
                ret.type = "file";
                let match = name.match(/^([\s\S]*):([0-9]+)$/);
                console.assert(match);
                ret.name = match[1];
                ret.size = Math.ceil(parseInt(match[2]) * (1/4096)) * 4096;
            } break;
            case "v": ret.type = "vetted"; break;
            case "x": ret.type = "excluded"; break;
            case "e": ret.type = "error_dir"; break;
            case "m": ret.type = "mount"; break;
            default:
                throw "unknown type: " + el[0];
            }
            ret.path = find_path(ret);
            return ret;
        }
    };
    for(let n = 0; n < raw_tree.length; ++n) {
        tree[n] = convert(raw_tree[n], null);
        raw_tree[n] = null;
    }
    raw_tree = null;
    if(errors.length > 0) {
        let error_node = document.createElement("pre");
        error_node.classList.add("errors");
        error_node.innerText = errors;
        document.body.appendChild(error_node);
        document.body.appendChild(document.createElement("hr"));
    }
    for(let n = 0; n < tree.length; ++n) {
        tree[n].nodes = make_nodes(tree[n]);
        document.body.appendChild(tree[n].nodes.div);
        if(tree[n].type == "dir") {
            disclose(tree[n]);
        }
    }
    document.body.appendChild(document.createElement("hr"));
    document.body.appendChild(excl_list);
    document.body.appendChild(document.createElement("hr"));
    document.body.appendChild(vet_list);
}
//--></script>
"###)?;
    }
    out.write_all(br###"</body>
</html>
"###)?;
    Ok(())
}
