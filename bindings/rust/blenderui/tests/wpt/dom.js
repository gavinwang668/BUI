// The DOM a WPT page's scripts see (tests/wpt/script.rs): the page's tree as JS objects, the
// mutations, selector queries and timers reftests use, and layout read from a render of the page
// as it loaded (`__layout`: element serial -> [x, y, width, height]). The harness reads the tree
// back (`__encode`) and renders it.
"use strict";
(function (g) {
  const HTML = "http://www.w3.org/1999/xhtml";
  const layout = JSON.parse(g.__layout);
  let serials = g.__serials;
  const log = (...a) => g.__log(a.map(String).join(" "));

  // -- nodes -------------------------------------------------------------------------------------
  class Node_ {
    constructor(type) {
      this.nodeType = type;
      this.parentNode = null;
      this.childNodes = [];
    }
    get firstChild() { return this.childNodes[0] || null; }
    get lastChild() { return this.childNodes[this.childNodes.length - 1] || null; }
    get nextSibling() {
      const p = this.parentNode;
      return p ? p.childNodes[p.childNodes.indexOf(this) + 1] || null : null;
    }
    get previousSibling() {
      const p = this.parentNode;
      return p ? p.childNodes[p.childNodes.indexOf(this) - 1] || null : null;
    }
    get parentElement() { return this.parentNode && this.parentNode.nodeType === 1 ? this.parentNode : null; }
    get ownerDocument() { return this.nodeType === 9 ? null : doc; }
    get isConnected() {
      let n = this;
      while (n.parentNode) n = n.parentNode;
      return n === doc;
    }
    get nodeValue() { return this.nodeType === 3 || this.nodeType === 8 ? this.data : null; }
    set nodeValue(v) { if (this.nodeType === 3 || this.nodeType === 8) this.data = String(v); }
    getRootNode() {
      let n = this;
      while (n.parentNode) n = n.parentNode;
      return n;
    }
    hasChildNodes() { return this.childNodes.length > 0; }
    contains(o) {
      for (; o; o = o.parentNode) if (o === this) return true;
      return false;
    }
    // the nodes inserting `n` inserts: a fragment's children, or `n` out of its old place
    _take(n) {
      if (typeof n === "string") n = doc.createTextNode(n);
      if (n.nodeType === 11) {
        const kids = n.childNodes;
        n.childNodes = [];
        return kids;
      }
      if (n.parentNode) n.parentNode.removeChild(n);
      return [n];
    }
    appendChild(n) {
      for (const c of this._take(n)) {
        c.parentNode = this;
        this.childNodes.push(c);
      }
      return n;
    }
    insertBefore(n, ref) {
      if (!ref) return this.appendChild(n);
      const list = this._take(n);
      const at = this.childNodes.indexOf(ref);
      this.childNodes.splice(at < 0 ? this.childNodes.length : at, 0, ...list);
      for (const c of list) c.parentNode = this;
      return n;
    }
    removeChild(n) {
      const at = this.childNodes.indexOf(n);
      if (at >= 0) {
        this.childNodes.splice(at, 1);
        n.parentNode = null;
      }
      return n;
    }
    replaceChild(n, old) {
      this.insertBefore(n, old);
      return this.removeChild(old);
    }
    remove() { if (this.parentNode) this.parentNode.removeChild(this); }
    append(...ns) { for (const n of ns) this.appendChild(n); }
    prepend(...ns) {
      const first = this.firstChild;
      for (const n of ns) this.insertBefore(n, first);
    }
    replaceChildren(...ns) {
      for (const c of this.childNodes) c.parentNode = null;
      this.childNodes = [];
      this.append(...ns);
    }
    before(...ns) {
      const p = this.parentNode;
      if (p) for (const n of ns) p.insertBefore(n, this);
    }
    after(...ns) {
      const p = this.parentNode;
      if (!p) return;
      const next = this.nextSibling;
      for (const n of ns) p.insertBefore(n, next);
    }
    replaceWith(...ns) {
      if (!this.parentNode) return;
      this.before(...ns);
      this.remove();
    }
    get textContent() {
      if (this.nodeType === 3 || this.nodeType === 8) return this.data;
      return this.childNodes.map((c) => (c.nodeType === 8 ? "" : c.textContent)).join("");
    }
    set textContent(t) {
      if (this.nodeType === 3 || this.nodeType === 8) {
        this.data = String(t);
        return;
      }
      for (const c of this.childNodes) c.parentNode = null;
      this.childNodes = [];
      if (t !== null && t !== undefined && t !== "") this.appendChild(doc.createTextNode(String(t)));
    }
    normalize() {}
    addEventListener(type, f) {
      if (!f) return;
      this._ev = this._ev || {};
      (this._ev[type] = this._ev[type] || []).push(f);
    }
    removeEventListener(type, f) {
      const l = this._ev && this._ev[type];
      if (l) this._ev[type] = l.filter((x) => x !== f);
    }
    dispatchEvent(e) {
      e.target = e.target || this;
      e.currentTarget = this;
      for (const f of (this._ev && this._ev[e.type]) || []) {
        try {
          typeof f === "function" ? f.call(this, e) : f.handleEvent(e);
        } catch (x) {
          log("listener:", x);
        }
      }
      const on = this["on" + e.type];
      if (typeof on === "function") on.call(this, e);
      return true;
    }
    isSameNode(o) { return o === this; }
    compareDocumentPosition(o) {
      if (o === this) return 0;
      const order = [...walk(this.getRootNode(), true)];
      return order.indexOf(o) < order.indexOf(this) ? 2 : 4;
    }
  }

  class CharacterData_ extends Node_ {
    constructor(type, data) {
      super(type);
      this.data = String(data);
    }
    get length() { return this.data.length; }
    appendData(s) { this.data += s; }
    insertData(at, s) { this.data = this.data.slice(0, at) + s + this.data.slice(at); }
    deleteData(at, n) { this.data = this.data.slice(0, at) + this.data.slice(at + n); }
    replaceData(at, n, s) { this.data = this.data.slice(0, at) + s + this.data.slice(at + n); }
    substringData(at, n) { return this.data.substr(at, n); }
  }

  class Text_ extends CharacterData_ {
    constructor(data) { super(3, data); }
    get nodeName() { return "#text"; }
    get wholeText() { return this.data; }
    splitText(at) {
      const rest = new Text_(this.data.slice(at));
      this.data = this.data.slice(0, at);
      if (this.parentNode) this.parentNode.insertBefore(rest, this.nextSibling);
      return rest;
    }
    cloneNode() { return new Text_(this.data); }
  }

  class Comment_ extends CharacterData_ {
    constructor(data) { super(8, data); }
    get nodeName() { return "#comment"; }
    cloneNode() { return new Comment_(this.data); }
  }

  // querySelector & co., for documents, fragments and elements
  const Queries = (Base) => class extends Base {
    querySelectorAll(s) {
      const m = selector(s);
      return [...walk(this, false)].filter((e) => e.nodeType === 1 && m(e));
    }
    querySelector(s) {
      const m = selector(s);
      for (const e of walk(this, false)) if (e.nodeType === 1 && m(e)) return e;
      return null;
    }
    getElementsByTagName(t) {
      t = String(t).toLowerCase();
      return [...walk(this, false)].filter((e) => e.nodeType === 1 && (t === "*" || e.localName.toLowerCase() === t));
    }
    getElementsByClassName(c) {
      const want = String(c).split(/\s+/).filter(Boolean);
      return [...walk(this, false)].filter((e) => e.nodeType === 1 && want.every((w) => e.classList.contains(w)));
    }
    getElementById(id) {
      for (const e of walk(this, false)) if (e.nodeType === 1 && e.getAttribute("id") === id) return e;
      return null;
    }
    get children() { return this.childNodes.filter((c) => c.nodeType === 1); }
    get firstElementChild() { return this.children[0] || null; }
    get lastElementChild() { const c = this.children; return c[c.length - 1] || null; }
    get childElementCount() { return this.children.length; }
  };

  class Fragment_ extends Queries(Node_) {
    constructor() { super(11); }
    get nodeName() { return "#document-fragment"; }
    cloneNode(deep) {
      const f = new Fragment_();
      if (deep) for (const c of this.childNodes) f.appendChild(c.cloneNode(true));
      return f;
    }
  }

  // -- elements ----------------------------------------------------------------------------------
  class Element_ extends Queries(Node_) {
    constructor(tag, ns, serial) {
      super(1);
      this.localName = tag;
      this.namespaceURI = ns || HTML;
      this._attrs = [];
      this._k = serial === undefined ? serials++ : serial;
    }
    get tagName() { return this.namespaceURI === HTML ? this.localName.toUpperCase() : this.localName; }
    get nodeName() { return this.tagName; }
    _name(n) { n = String(n); return this.namespaceURI === HTML ? n.toLowerCase() : n; }
    getAttribute(n) {
      n = this._name(n);
      const a = this._attrs.find((a) => a[0] === n);
      return a ? a[1] : null;
    }
    setAttribute(n, v) {
      n = this._name(n);
      v = String(v);
      const a = this._attrs.find((a) => a[0] === n);
      if (a) a[1] = v;
      else this._attrs.push([n, v]);
    }
    removeAttribute(n) {
      n = this._name(n);
      this._attrs = this._attrs.filter((a) => a[0] !== n);
    }
    hasAttribute(n) { return this.getAttribute(n) !== null; }
    hasAttributes() { return this._attrs.length > 0; }
    toggleAttribute(n, force) {
      const has = this.hasAttribute(n);
      const want = force === undefined ? !has : !!force;
      if (want && !has) this.setAttribute(n, "");
      if (!want && has) this.removeAttribute(n);
      return want;
    }
    getAttributeNS(_, n) { return this.getAttribute(n); }
    setAttributeNS(_, n, v) { this.setAttribute(String(n).replace(/^.*:/, ""), v); }
    removeAttributeNS(_, n) { this.removeAttribute(n); }
    hasAttributeNS(_, n) { return this.hasAttribute(n); }
    getAttributeNames() { return this._attrs.map((a) => a[0]); }
    get attributes() {
      const list = this._attrs.map(([name, value]) => ({ name, value, localName: name, nodeName: name }));
      list.getNamedItem = (n) => list.find((a) => a.name === n) || null;
      return list;
    }
    get id() { return this.getAttribute("id") || ""; }
    set id(v) { this.setAttribute("id", v); }
    get className() { return this.getAttribute("class") || ""; }
    set className(v) { this.setAttribute("class", v); }
    get classList() { return new TokenList_(this, "class"); }
    get style() { return styleOf(this); }
    set style(v) { this.setAttribute("style", v); }
    get dataset() {
      const el = this;
      const attr = (k) => "data-" + String(k).replace(/[A-Z]/g, (m) => "-" + m.toLowerCase());
      return new Proxy({}, {
        get: (_, k) => (typeof k === "string" ? el.getAttribute(attr(k)) ?? undefined : undefined),
        set: (_, k, v) => (el.setAttribute(attr(k), v), true),
        deleteProperty: (_, k) => (el.removeAttribute(attr(k)), true),
      });
    }
    get nextElementSibling() {
      let n = this.nextSibling;
      while (n && n.nodeType !== 1) n = n.nextSibling;
      return n;
    }
    get previousElementSibling() {
      let n = this.previousSibling;
      while (n && n.nodeType !== 1) n = n.previousSibling;
      return n;
    }
    cloneNode(deep) {
      const e = new Element_(this.localName, this.namespaceURI);
      e._attrs = this._attrs.map((a) => [...a]);
      if (deep) for (const c of this.childNodes) e.appendChild(c.cloneNode(true));
      return e;
    }
    matches(s) { return selector(s)(this); }
    webkitMatchesSelector(s) { return this.matches(s); }
    closest(s) {
      const m = selector(s);
      for (let e = this; e && e.nodeType === 1; e = e.parentNode) if (m(e)) return e;
      return null;
    }
    get innerHTML() { return this.childNodes.map(html).join(""); }
    set innerHTML(h) {
      this.textContent = "";
      for (const n of parse(h)) this.appendChild(n);
    }
    get outerHTML() { return html(this); }
    set outerHTML(h) {
      if (!this.parentNode) return;
      for (const n of parse(h)) this.parentNode.insertBefore(n, this);
      this.remove();
    }
    get innerText() { return this.textContent; }
    set innerText(t) { this.textContent = t; }
    insertAdjacentHTML(where, h) { this.insertAdjacentElement(where, ...parse(h)); }
    insertAdjacentText(where, t) { this.insertAdjacentElement(where, doc.createTextNode(t)); }
    insertAdjacentElement(where, ...ns) {
      switch (String(where).toLowerCase()) {
        case "beforebegin": this.before(...ns); break;
        case "afterbegin": this.prepend(...ns); break;
        case "beforeend": this.append(...ns); break;
        case "afterend": this.after(...ns); break;
      }
      return ns[0];
    }
    // layout: as the page was laid out before its scripts ran
    _box() { return layout[this._k] || [0, 0, 0, 0]; }
    getBoundingClientRect() {
      const [x, y, width, height] = this._box();
      return { x, y, width, height, left: x, top: y, right: x + width, bottom: y + height, toJSON() { return this; } };
    }
    getClientRects() { return [this.getBoundingClientRect()]; }
    get offsetParent() { return doc.body; }
    get offsetLeft() { return Math.round(this._box()[0]); }
    get offsetTop() { return Math.round(this._box()[1]); }
    get offsetWidth() { return Math.round(this._box()[2]); }
    get offsetHeight() { return Math.round(this._box()[3]); }
    get clientLeft() { return 0; }
    get clientTop() { return 0; }
    get clientWidth() { return Math.round(this._box()[2]); }
    get clientHeight() { return Math.round(this._box()[3]); }
    get scrollWidth() { return Math.round(this._box()[2]); }
    get scrollHeight() { return Math.round(this._box()[3]); }
    // scrolling: kept for the render
    get scrollLeft() { return this._scroll ? this._scroll[0] : 0; }
    set scrollLeft(v) { this._scroll = [Math.max(0, +v || 0), this.scrollTop]; }
    get scrollTop() { return this._scroll ? this._scroll[1] : 0; }
    set scrollTop(v) { this._scroll = [this.scrollLeft, Math.max(0, +v || 0)]; }
    scrollTo(x, y) {
      if (x && typeof x === "object") {
        if (x.left !== undefined) this.scrollLeft = x.left;
        if (x.top !== undefined) this.scrollTop = x.top;
      } else {
        this.scrollLeft = x;
        this.scrollTop = y;
      }
    }
    scroll(x, y) { this.scrollTo(x, y); }
    scrollBy(x, y) {
      if (x && typeof x === "object") this.scrollTo(this.scrollLeft + (x.left || 0), this.scrollTop + (x.top || 0));
      else this.scrollTo(this.scrollLeft + (+x || 0), this.scrollTop + (+y || 0));
    }
    scrollIntoView() {}
    focus() { focused = this; }
    blur() { if (focused === this) focused = null; }
    click() { this.dispatchEvent(new Event_("click")); }
    animate() { return animation(); }
    getAnimations() { return []; }
    attachShadow() { throw new Error("shadow DOM is not supported"); }
    get shadowRoot() { return null; }
    get sheet() { return null; }
    // form controls
    get value() { return this.getAttribute("value") ?? (this.localName === "textarea" ? this.textContent : ""); }
    set value(v) { this.localName === "textarea" ? (this.textContent = v) : this.setAttribute("value", v); }
    get checked() { return this.hasAttribute("checked"); }
    set checked(v) { this.toggleAttribute("checked", !!v); }
    get disabled() { return this.hasAttribute("disabled"); }
    set disabled(v) { this.toggleAttribute("disabled", !!v); }
    get hidden() { return this.hasAttribute("hidden"); }
    set hidden(v) { this.toggleAttribute("hidden", !!v); }
    get open() { return this.hasAttribute("open"); }
    set open(v) { this.toggleAttribute("open", !!v); }
    get src() { return this.getAttribute("src") || ""; }
    set src(v) { this.setAttribute("src", v); }
    get href() { return this.getAttribute("href") || ""; }
    set href(v) { this.setAttribute("href", v); }
    get dir() { return this.getAttribute("dir") || ""; }
    set dir(v) { this.setAttribute("dir", v); }
    get type() { return (this.getAttribute("type") || (this.localName === "input" ? "text" : "")).toLowerCase(); }
    set type(v) { this.setAttribute("type", v); }
    get placeholder() { return this.getAttribute("placeholder") || ""; }
    set placeholder(v) { this.setAttribute("placeholder", v); }
    get required() { return this.hasAttribute("required"); }
    set required(v) { this.toggleAttribute("required", !!v); }
    get readOnly() { return this.hasAttribute("readonly"); }
    set readOnly(v) { this.toggleAttribute("readonly", !!v); }
    get lang() { return this.getAttribute("lang") || ""; }
    set lang(v) { this.setAttribute("lang", v); }
    get title() { return this.getAttribute("title") || ""; }
    set title(v) { this.setAttribute("title", v); }
    get width() { return +(this.getAttribute("width") || 0); }
    set width(v) { this.setAttribute("width", v); }
    get height() { return +(this.getAttribute("height") || 0); }
    set height(v) { this.setAttribute("height", v); }
    get content() {
      const f = new Fragment_();
      for (const c of this.childNodes) f.appendChild(c.cloneNode(true));
      return f;
    }
    // a canvas's 2D context: rectangle fills, which `__encode` turns into boxes
    getContext(kind) {
      if (this.localName !== "canvas" || kind !== "2d") return null;
      if (!this._ctx) this._ctx = context2d(this);
      return this._ctx;
    }
    showModal() {
      this.setAttribute("open", "");
      popovers.add(this);
    }
    show() { this.setAttribute("open", ""); }
    close() {
      this.removeAttribute("open");
      popovers.delete(this);
    }
    showPopover() { popovers.add(this); }
    hidePopover() { popovers.delete(this); }
    togglePopover(force) {
      const show = force === undefined ? !popovers.has(this) : !!force;
      show ? popovers.add(this) : popovers.delete(this);
      return show;
    }
  }

  class TokenList_ {
    constructor(el, attr) {
      this._el = el;
      this._a = attr;
    }
    get _v() { return (this._el.getAttribute(this._a) || "").split(/\s+/).filter(Boolean); }
    _set(v) { this._el.setAttribute(this._a, v.join(" ")); }
    add(...t) {
      const v = this._v;
      for (const x of t) if (!v.includes(x)) v.push(x);
      this._set(v);
    }
    remove(...t) { if (this._el.hasAttribute(this._a)) this._set(this._v.filter((x) => !t.includes(x))); }
    toggle(t, force) {
      const has = this.contains(t);
      const want = force === undefined ? !has : !!force;
      if (want && !has) this.add(t);
      if (!want && has) this.remove(t);
      return want;
    }
    contains(t) { return this._v.includes(t); }
    replace(a, b) {
      const v = this._v;
      const at = v.indexOf(a);
      if (at < 0) return false;
      v[at] = b;
      this._set([...new Set(v)]);
      return true;
    }
    item(i) { return this._v[i] ?? null; }
    get length() { return this._v.length; }
    get value() { return this._el.getAttribute(this._a) || ""; }
    set value(s) { this._el.setAttribute(this._a, s); }
    toString() { return this.value; }
    forEach(f) { this._v.forEach(f); }
    [Symbol.iterator]() { return this._v[Symbol.iterator](); }
  }

  // -- inline style: a view of the `style` attribute ---------------------------------------------
  const propName = (p) => {
    p = String(p);
    if (p.startsWith("--")) return p;
    if (p === "cssFloat") return "float";
    p = p.replace(/[A-Z]/g, (m) => "-" + m.toLowerCase());
    return /^(webkit|moz|ms)-/.test(p) ? "-" + p : p;
  };
  function declarations(text) {
    const out = [];
    const push = (s) => {
      const at = s.indexOf(":");
      if (at < 0) return;
      const name = s.slice(0, at).trim();
      let value = s.slice(at + 1).trim();
      const imp = /!\s*important\s*$/i.exec(value);
      if (imp) value = value.slice(0, imp.index).trim();
      if (name) out.push([name.startsWith("--") ? name : name.toLowerCase(), value, !!imp]);
    };
    let depth = 0, quote = null, start = 0;
    for (let i = 0; i < text.length; i++) {
      const c = text[i];
      if (quote) {
        if (c === "\\") i++;
        else if (c === quote) quote = null;
      } else if (c === '"' || c === "'") quote = c;
      else if (c === "(") depth++;
      else if (c === ")") depth--;
      else if (c === ";" && depth <= 0) {
        push(text.slice(start, i));
        start = i + 1;
      }
    }
    push(text.slice(start));
    return out;
  }
  class Style_ {
    constructor(el) { this._el = el; }
    get _d() { return declarations(this._el.getAttribute("style") || ""); }
    _write(d) { this._el.setAttribute("style", d.map(([n, v, i]) => `${n}: ${v}${i ? " !important" : ""};`).join(" ")); }
    getPropertyValue(p) {
      p = propName(p);
      const d = this._d.filter((x) => x[0] === p).pop();
      return d ? d[1] : "";
    }
    getPropertyPriority(p) {
      p = propName(p);
      const d = this._d.filter((x) => x[0] === p).pop();
      return d && d[2] ? "important" : "";
    }
    setProperty(p, v, prio) {
      p = propName(p);
      v = v === null || v === undefined ? "" : String(v).trim();
      if (v === "") {
        this.removeProperty(p);
        return;
      }
      const d = this._d.filter((x) => x[0] !== p);
      d.push([p, v, String(prio || "").toLowerCase() === "important"]);
      this._write(d);
    }
    removeProperty(p) {
      const old = this.getPropertyValue(p);
      p = propName(p);
      this._write(this._d.filter((x) => x[0] !== p));
      return old;
    }
    get cssText() { return this._el.getAttribute("style") || ""; }
    set cssText(t) { this._write(declarations(String(t))); }
    get length() { return this._d.length; }
    item(i) { const d = this._d[i]; return d ? d[0] : ""; }
  }
  function styleOf(el) {
    return new Proxy(new Style_(el), {
      get(t, k) {
        if (typeof k !== "string" || k in t) {
          const v = t[k];
          return typeof v === "function" ? v.bind(t) : v;
        }
        return /^\d+$/.test(k) ? t.item(+k) : t.getPropertyValue(k);
      },
      set(t, k, v) {
        if (typeof k === "string" && !(k in t)) t.setProperty(k, v);
        else t[k] = v;
        return true;
      },
    });
  }
  // computed style: the inline value, or the laid-out size; enough for the reads that force a
  // style or layout update
  function getComputedStyle(el) {
    const own = el && el.nodeType === 1 ? styleOf(el) : null;
    const read = (p) => {
      p = propName(p);
      if (!own) return "";
      if (p === "width") return el._box()[2] + "px";
      if (p === "height") return el._box()[3] + "px";
      return own.getPropertyValue(p);
    };
    return new Proxy({}, {
      get(_, k) {
        if (k === "getPropertyValue") return read;
        if (k === "getPropertyPriority") return () => "";
        if (k === "length") return 0;
        return typeof k === "string" ? read(k) : undefined;
      },
    });
  }

  // -- selectors ---------------------------------------------------------------------------------
  const compiled = new Map();
  function selector(src) {
    src = String(src);
    let m = compiled.get(src);
    if (!m) {
      m = compileList(src);
      compiled.set(src, m);
    }
    return m;
  }
  function nth(arg) {
    arg = arg.trim().toLowerCase().replace(/\s+/g, "");
    if (arg === "odd") return [2, 1];
    if (arg === "even") return [2, 0];
    const m = /^([+-]?\d*)n([+-]\d+)?$/.exec(arg);
    if (m) return [m[1] === "" || m[1] === "+" ? 1 : m[1] === "-" ? -1 : +m[1], m[2] ? +m[2] : 0];
    return [0, +arg];
  }
  const fits = ([a, b], i) => (a === 0 ? i === b : (i - b) / a >= 0 && (i - b) % a === 0);
  const siblings = (e) => (e.parentNode ? e.parentNode.childNodes.filter((c) => c.nodeType === 1) : [e]);
  function pseudo(name, arg) {
    switch (name) {
      case "not": { const m = compileList(arg); return (e) => !m(e); }
      case "is": case "where": case "matches": case "any": return compileList(arg);
      case "has": {
        const m = compileList(arg.replace(/^\s*[>+~]/, "").trim());
        return (e) => [...walk(e, false)].some((d) => d.nodeType === 1 && m(d));
      }
      case "first-child": return (e) => siblings(e)[0] === e;
      case "last-child": return (e) => { const s = siblings(e); return s[s.length - 1] === e; };
      case "only-child": return (e) => siblings(e).length === 1;
      case "nth-child": { const f = nth(arg); return (e) => fits(f, siblings(e).indexOf(e) + 1); }
      case "nth-last-child": { const f = nth(arg); return (e) => { const s = siblings(e); return fits(f, s.length - s.indexOf(e)); }; }
      case "first-of-type": return (e) => siblings(e).find((s) => s.localName === e.localName) === e;
      case "last-of-type": return (e) => siblings(e).filter((s) => s.localName === e.localName).pop() === e;
      case "nth-of-type": { const f = nth(arg); return (e) => fits(f, siblings(e).filter((s) => s.localName === e.localName).indexOf(e) + 1); }
      case "root": return (e) => e === doc.documentElement;
      case "empty": return (e) => e.childNodes.every((c) => c.nodeType === 8 || (c.nodeType === 3 && c.data === ""));
      case "checked": return (e) => e.hasAttribute("checked") || e.hasAttribute("selected");
      case "disabled": return (e) => e.hasAttribute("disabled");
      case "enabled": return (e) => !e.hasAttribute("disabled");
      case "scope": return (e) => e === doc.documentElement;
      default: return () => false;
    }
  }
  function attrTest(name, op, val, ci) {
    const norm = (s) => (ci ? s.toLowerCase() : s);
    return (e) => {
      let v = e.getAttribute(name);
      if (v === null) return false;
      if (!op) return true;
      v = norm(v);
      const w = norm(val);
      switch (op) {
        case "=": return v === w;
        case "~=": return v.split(/\s+/).includes(w);
        case "|=": return v === w || v.startsWith(w + "-");
        case "^=": return w !== "" && v.startsWith(w);
        case "$=": return w !== "" && v.endsWith(w);
        case "*=": return w !== "" && v.includes(w);
      }
      return false;
    };
  }
  function compileList(src) {
    let i = 0;
    const ws = () => { while (i < src.length && /\s/.test(src[i])) i++; };
    const ident = () => {
      let s = "";
      while (i < src.length && /[\w\- -￿\\]/.test(src[i])) {
        if (src[i] === "\\") i++;
        s += src[i++];
      }
      return s;
    };
    function compound() {
      const tests = [];
      if (src[i] === "*") i++;
      else if (/[\w\-\\]/.test(src[i] || "")) {
        const t = ident().toLowerCase();
        tests.push((e) => e.localName.toLowerCase() === t);
      }
      for (;;) {
        const c = src[i];
        if (c === "#") {
          i++;
          const v = ident();
          tests.push((e) => e.getAttribute("id") === v);
        } else if (c === ".") {
          i++;
          const v = ident();
          tests.push((e) => (e.getAttribute("class") || "").split(/\s+/).includes(v));
        } else if (c === "[") {
          i++;
          ws();
          const n = ident().toLowerCase();
          ws();
          let op = "";
          while ("~|^$*=".includes(src[i]) && src[i]) op += src[i++];
          ws();
          let v = null;
          if (op) {
            if (src[i] === '"' || src[i] === "'") {
              const q = src[i++];
              v = "";
              while (i < src.length && src[i] !== q) v += src[i++];
              i++;
            } else v = ident();
          }
          ws();
          let ci = false;
          if (/[iIsS]/.test(src[i] || "")) {
            ci = /[iI]/.test(src[i]);
            i++;
            ws();
          }
          if (src[i] === "]") i++;
          tests.push(attrTest(n, op, v, ci));
        } else if (c === ":") {
          i++;
          if (src[i] === ":") {
            i++;
            ident();
            tests.push(() => false);
            continue;
          }
          const name = ident().toLowerCase();
          let arg = null;
          if (src[i] === "(") {
            let depth = 1;
            const start = ++i;
            while (i < src.length && depth) {
              if (src[i] === "(") depth++;
              else if (src[i] === ")") depth--;
              i++;
            }
            arg = src.slice(start, i - 1);
          }
          tests.push(pseudo(name, arg));
        } else break;
      }
      return (e) => tests.every((t) => t(e));
    }
    function complex() {
      const parts = [compound()];
      const combs = [];
      for (;;) {
        const save = i;
        ws();
        const c = src[i];
        if (c === undefined || c === ",") break;
        let comb = " ";
        if (">+~".includes(c)) {
          comb = c;
          i++;
          ws();
        } else if (i === save) break;
        parts.push(compound());
        combs.push(comb);
      }
      return (e) => from(parts, combs, parts.length - 1, e);
    }
    const list = [];
    ws();
    for (;;) {
      list.push(complex());
      ws();
      if (src[i] !== ",") break;
      i++;
      ws();
    }
    if (i < src.length) throw new SyntaxError("selector: " + src);
    return (e) => list.some((m) => m(e));
  }
  function from(parts, combs, k, e) {
    if (!parts[k](e)) return false;
    if (k === 0) return true;
    switch (combs[k - 1]) {
      case ">": { const p = e.parentElement; return !!p && from(parts, combs, k - 1, p); }
      case "+": { const p = e.previousElementSibling; return !!p && from(parts, combs, k - 1, p); }
      case "~":
        for (let p = e.previousElementSibling; p; p = p.previousElementSibling) if (from(parts, combs, k - 1, p)) return true;
        return false;
      default:
        for (let p = e.parentElement; p; p = p.parentElement) if (from(parts, combs, k - 1, p)) return true;
        return false;
    }
  }

  // -- canvas: what reftests draw (fillRect), kept as rectangles in canvas pixels ------------------
  function context2d(canvas) {
    const ops = [];
    canvas._ops = ops;
    let m = [1, 0, 0, 1, 0, 0];
    const stack = [];
    const ctx = {
      canvas, fillStyle: "#000", strokeStyle: "#000", globalAlpha: 1, lineWidth: 1, font: "10px sans-serif",
      save() { stack.push([m.slice(), ctx.fillStyle, ctx.globalAlpha]); },
      restore() { const s = stack.pop(); if (s) [m, ctx.fillStyle, ctx.globalAlpha] = s; },
      scale(x, y) { m = [m[0] * x, m[1] * x, m[2] * y, m[3] * y, m[4], m[5]]; },
      translate(x, y) { m = [m[0], m[1], m[2], m[3], m[4] + m[0] * x + m[2] * y, m[5] + m[1] * x + m[3] * y]; },
      setTransform(a, b, c, d, e, f) { m = [a, b, c, d, e, f]; },
      resetTransform() { m = [1, 0, 0, 1, 0, 0]; },
      fillRect(x, y, w, h) {
        // axis-aligned transforms only (what reftests use)
        const x0 = m[0] * x + m[4], y0 = m[3] * y + m[5];
        ops.push([Math.min(x0, x0 + m[0] * w), Math.min(y0, y0 + m[3] * h), Math.abs(m[0] * w), Math.abs(m[3] * h),
                  typeof ctx.fillStyle === "string" ? ctx.fillStyle : "transparent", ctx.globalAlpha]);
      },
      clearRect(x, y, w, h) {
        const x0 = m[0] * x + m[4], y0 = m[3] * y + m[5];
        ops.push([x0, y0, m[0] * w, m[3] * h, "white", 1]);
      },
      createLinearGradient: () => ({ addColorStop() {} }),
      createRadialGradient: () => ({ addColorStop() {} }),
      createPattern: () => null,
      beginPath() {}, closePath() {}, moveTo() {}, lineTo() {}, rect() {}, arc() {}, fill() {}, stroke() {},
      strokeRect() {}, clip() {}, fillText() {}, strokeText() {}, drawImage() {},
      measureText: (t) => ({ width: String(t).length * 5 }),
      getImageData: (x, y, w, h) => ({ width: w, height: h, data: new Uint8ClampedArray(w * h * 4) }),
      putImageData() {},
    };
    return ctx;
  }

  // -- trees: walking, (de)serialising -----------------------------------------------------------
  function* walk(root, self) {
    if (self) yield root;
    for (const c of root.childNodes) yield* walk(c, true);
  }
  const VOID = new Set(["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr"]);
  const esc = (s, attr) => s.replace(/&/g, "&amp;").replace(attr ? /"/g : /</g, attr ? "&quot;" : "&lt;").replace(/ /g, "&nbsp;");
  function html(n) {
    if (n.nodeType === 3) return n.parentNode && ["style", "script"].includes(n.parentNode.localName) ? n.data : esc(n.data, false);
    if (n.nodeType === 8) return `<!--${n.data}-->`;
    if (n.nodeType !== 1) return n.childNodes.map(html).join("");
    const attrs = n._attrs.map(([k, v]) => ` ${k}="${esc(v, true)}"`).join("");
    if (VOID.has(n.localName)) return `<${n.localName}${attrs}>`;
    return `<${n.localName}${attrs}>${n.childNodes.map(html).join("")}</${n.localName}>`;
  }
  // a tree from the harness (`{t, k, a, c}` elements and string texts)
  function build(j) {
    if (typeof j === "string") return new Text_(j);
    const e = new Element_(j.t, j.n, j.k);
    e._attrs = j.a;
    for (const c of j.c) e.appendChild(build(c));
    return e;
  }
  const parse = (h) => JSON.parse(g.__parse(String(h))).map(build);
  // the tree for the harness: preorder, U+0001-separated
  g.__encode = () => {
    const out = [];
    const put = (n) => {
      if (n.nodeType === 3) {
        out.push("T", n.data);
        return;
      }
      if (n._ops && n._ops.length) {
        const [w, h] = [n.getAttribute("width") || 300, n.getAttribute("height") || 150];
        const own = n.getAttribute("style") || "";
        const attrs = n._attrs.filter((a) => a[0] !== "style");
        out.push("E", "div", String(n._k), String(attrs.length + 1));
        for (const [k, v] of attrs) out.push(k, v);
        out.push("style", `display: inline-block; position: relative; overflow: hidden; width: ${w}px; height: ${h}px; ${own}`);
        out.push("", String(n._ops.length));
        for (const [x, y, ow, oh, fill, alpha] of n._ops) {
          out.push("E", "div", String(serials++), "1", "style",
                   `position: absolute; left: ${x}px; top: ${y}px; width: ${ow}px; height: ${oh}px; background: ${fill}; opacity: ${alpha}`, "", "0");
        }
        return;
      }
      out.push("E", n.localName, String(n._k), String(n._attrs.length));
      for (const [k, v] of n._attrs) out.push(k, v);
      out.push(n._scroll ? n._scroll.join(",") : "");
      const kids = n.childNodes.filter((c) => c.nodeType === 1 || c.nodeType === 3);
      out.push(String(kids.length));
      kids.forEach(put);
    };
    out.push(focused && focused.isConnected ? String(focused._k) : "");
    out.push([...popovers].filter((p) => p.isConnected).map((p) => p._k).join(","));
    put(doc.documentElement);
    return out.join("\u0001");
  };

  // -- the document and the window ---------------------------------------------------------------
  class Document_ extends Queries(Node_) {
    constructor() {
      super(9);
      this.readyState = "loading";
      this.fonts = {
        ready: Promise.resolve(),
        status: "loaded",
        load: () => Promise.resolve([]),
        check: () => true,
        add() {},
        delete() {},
        forEach() {},
        addEventListener() {},
        [Symbol.iterator]: function* () {},
      };
      this.styleSheets = [];
      this.compatMode = "CSS1Compat";
    }
    get nodeName() { return "#document"; }
    get documentElement() { return this.childNodes.find((c) => c.nodeType === 1) || null; }
    get body() {
      const h = this.documentElement;
      return (h && h.children.find((c) => c.localName === "body")) || null;
    }
    get head() {
      const h = this.documentElement;
      return (h && h.children.find((c) => c.localName === "head")) || null;
    }
    get scrollingElement() { return this.documentElement; }
    get defaultView() { return g; }
    get activeElement() { return focused || this.body; }
    createElement(t) { return new Element_(String(t).toLowerCase()); }
    createElementNS(ns, t) { return new Element_(String(t).replace(/^.*:/, ""), ns); }
    createTextNode(d) { return new Text_(d); }
    createComment(d) { return new Comment_(d); }
    createDocumentFragment() { return new Fragment_(); }
    createRange() {
      return { setStart() {}, setEnd() {}, selectNode() {}, selectNodeContents() {}, collapse() {},
               getBoundingClientRect: () => ({ x: 0, y: 0, width: 0, height: 0, top: 0, left: 0, right: 0, bottom: 0 }),
               getClientRects: () => [] };
    }
    createEvent() { return new Event_(""); }
    hasFocus() { return true; }
    elementFromPoint() { return null; }
    elementsFromPoint() { return []; }
    getSelection() { return g.getSelection(); }
    execCommand() { return false; }
    write(h) {
      const b = this.body || this.documentElement;
      if (b) for (const n of parse(h)) b.appendChild(n);
    }
    writeln(h) { this.write(h + "\n"); }
    open() {}
    close() {}
  }

  class Event_ {
    constructor(type, init) {
      this.type = type;
      Object.assign(this, init || {});
      this.defaultPrevented = false;
    }
    preventDefault() { this.defaultPrevented = true; }
    stopPropagation() {}
    stopImmediatePropagation() {}
    initEvent(type) { this.type = type; }
  }

  function animation() {
    return { finished: Promise.resolve(), ready: Promise.resolve(), currentTime: 0, playState: "finished",
             effect: { getComputedTiming: () => ({}), updateTiming() {} },
             cancel() {}, pause() {}, play() {}, finish() {}, reverse() {}, addEventListener() {} };
  }

  const doc = new Document_();
  let focused = null;
  const popovers = new Set();
  doc.appendChild(build(JSON.parse(g.__tree)));

  // timers: a virtual clock, run by the harness until nothing is left (`__step`)
  let now = 0, seq = 0;
  const timers = [];
  const later = (f, at, args) => {
    const id = ++seq;
    timers.push({ id, at, f: typeof f === "function" ? () => f(...args) : () => (0, eval)(String(f)) });
    return id;
  };
  const cancel = (id) => {
    const at = timers.findIndex((t) => t.id === id);
    if (at >= 0) timers.splice(at, 1);
  };
  const frame = () => (Math.floor(now / 16) + 1) * 16;
  // named access: an element's id is a global naming it, unless something else has the name
  const named = () => {
    for (const e of walk(doc, false)) {
      const id = e.nodeType === 1 && e.getAttribute("id");
      if (!id || id in g) continue;
      Object.defineProperty(g, id, {
        configurable: true,
        get: () => doc.getElementById(id),
        set: (v) => Object.defineProperty(g, id, { value: v, writable: true, configurable: true, enumerable: true }),
      });
    }
  };
  g.__step = () => {
    if (!timers.length) return false;
    named();
    timers.sort((a, b) => a.at - b.at || a.id - b.id);
    const t = timers.shift();
    now = Math.max(now, t.at);
    try {
      t.f();
    } catch (e) {
      log("timer:", e, e && e.stack);
    }
    return true;
  };

  const win = {
    window: g, self: g, top: g, parent: g, frames: g, globalThis: g, document: doc,
    innerWidth: 800, innerHeight: 600, outerWidth: 800, outerHeight: 600, devicePixelRatio: 1,
    scrollX: 0, scrollY: 0, pageXOffset: 0, pageYOffset: 0,
    location: { href: "about:blank", search: "", hash: "", protocol: "http:", host: "web-platform.test", pathname: "/", origin: "http://web-platform.test", reload() {} },
    navigator: { userAgent: "BUI", platform: "", language: "en", languages: ["en"] },
    screen: { width: 800, height: 600 },
    history: { pushState() {}, replaceState() {} },
    console: { log, error: log, warn: log, info: log, debug() {} },
    setTimeout: (f, ms, ...args) => later(f, now + Math.max(0, +ms || 0), args),
    setInterval: (f, ms, ...args) => {
      const id = ++seq;
      const step = Math.max(1, +ms || 0);
      let runs = 0;
      const tick = () => {
        if (++runs > 100) return;
        timers.push({ id, at: now + step, f: tick });
        typeof f === "function" ? f(...args) : (0, eval)(String(f));
      };
      timers.push({ id, at: now + step, f: tick });
      return id;
    },
    clearTimeout: cancel,
    clearInterval: cancel,
    requestAnimationFrame: (f) => later(() => f(now), frame(), []),
    cancelAnimationFrame: cancel,
    requestIdleCallback: (f) => later(() => f({ timeRemaining: () => 50, didTimeout: false }), now + 1, []),
    cancelIdleCallback: cancel,
    queueMicrotask: (f) => Promise.resolve().then(f),
    performance: { now: () => now, mark() {}, measure() {} },
    getComputedStyle,
    matchMedia: (q) => ({ matches: false, media: q, addListener() {}, removeListener() {}, addEventListener() {}, removeEventListener() {} }),
    getSelection: () => ({ removeAllRanges() {}, addRange() {}, selectAllChildren() {}, collapse() {}, toString: () => "" }),
    scrollTo(x, y) { doc.documentElement.scrollTo(x, y); },
    scroll(x, y) { doc.documentElement.scrollTo(x, y); },
    scrollBy(x, y) { doc.documentElement.scrollBy(x, y); },
    addEventListener(type, f) { winNode.addEventListener(type, f); },
    removeEventListener(type, f) { winNode.removeEventListener(type, f); },
    dispatchEvent(e) { return winNode.dispatchEvent(e); },
    alert() {}, confirm: () => true, prompt: () => null, focus() {}, blur() {}, close() {}, open: () => null, print() {},
    postMessage() {},
    Node: { ELEMENT_NODE: 1, TEXT_NODE: 3, COMMENT_NODE: 8, DOCUMENT_NODE: 9, DOCUMENT_FRAGMENT_NODE: 11 },
    Element: Element_, HTMLElement: Element_, SVGElement: Element_, Text: Text_, Comment: Comment_,
    DocumentFragment: Fragment_, Document: Document_, CharacterData: CharacterData_,
    Event: Event_, CustomEvent: Event_, MouseEvent: Event_, KeyboardEvent: Event_, UIEvent: Event_,
    CSS: { supports: () => true, escape: (s) => String(s), registerProperty() {}, px: (n) => n + "px" },
    customElements: { define() {}, get() {}, whenDefined: () => Promise.resolve() },
    ResizeObserver: class { observe() {} unobserve() {} disconnect() {} },
    IntersectionObserver: class { observe() {} unobserve() {} disconnect() {} takeRecords() { return []; } },
    MutationObserver: class { observe() {} disconnect() {} takeRecords() { return []; } },
    FontFace: class {
      constructor(family) { this.family = family; this.status = "loaded"; this.loaded = Promise.resolve(this); }
      load() { return Promise.resolve(this); }
    },
    Image: class extends Element_ { constructor() { super("img"); } },
    XMLHttpRequest: class { open() {} send() {} setRequestHeader() {} },
    fetch: () => Promise.reject(new Error("no network")),
  };
  win.Node = Object.assign(Node_, win.Node);
  // the window's listeners (load) live on a stand-in node
  const winNode = new Node_(0);
  for (const k in win) {
    try {
      g[k] = win[k];
    } catch (_) { /* a read-only global */ }
  }
  for (const t of ["HTMLDivElement", "HTMLSpanElement", "HTMLImageElement", "HTMLCanvasElement", "HTMLInputElement",
                   "HTMLStyleElement", "HTMLTemplateElement", "HTMLSlotElement", "HTMLIFrameElement", "HTMLBodyElement"]) {
    g[t] = Element_;
  }

  // the page: its scripts in order, then its load events (`__script(i)`, `__loaded()`)
  const scripts = [...walk(doc, false)].filter((e) => e.nodeType === 1 && e.localName === "script"
    && /^(|text\/javascript|application\/javascript|module)$/i.test((e.getAttribute("type") || "").trim()));
  g.__scripts = scripts.length;
  g.__script = (i) => {
    named();
    try {
      (0, eval)(scripts[i].textContent);
    } catch (e) {
      log("script:", e, e && e.stack);
    }
  };
  g.__loaded = () => {
    doc.readyState = "interactive";
    doc.dispatchEvent(new Event_("DOMContentLoaded"));
    doc.readyState = "complete";
    const body = doc.body;
    const inline = body && body.getAttribute("onload");
    const load = new Event_("load");
    if (inline) {
      try {
        new Function("event", inline).call(body, load);
      } catch (e) {
        log("onload:", e);
      }
    }
    winNode.onload = g.onload;
    winNode.dispatchEvent(load);
  };
  // testharness.js (some reftests load it): its API, doing nothing
  g.__testharness = () => {
    const t = { step: (f) => f && f(), step_func: (f) => f, step_func_done: (f) => (...a) => f && f(...a),
                done() {}, add_cleanup() {}, unreached_func: () => () => {}, step_timeout: (f, ms) => g.setTimeout(f, ms) };
    g.test = (f) => { try { f(t); } catch (_) { /* an assertion */ } };
    g.async_test = (f) => { if (typeof f === "function") try { f(t); } catch (_) { /* an assertion */ } return t; };
    g.promise_test = (f) => { try { Promise.resolve(f(t)).catch(() => {}); } catch (_) { /* an assertion */ } };
    g.setup = () => {};
    g.done = () => {};
    g.add_completion_callback = () => {};
    g.format_value = String;
    for (const k of ["assert_equals", "assert_not_equals", "assert_true", "assert_false", "assert_approx_equals",
                     "assert_array_equals", "assert_greater_than", "assert_less_than", "assert_throws_dom",
                     "assert_throws_js", "assert_unreached", "assert_in_array", "assert_class_string",
                     "assert_greater_than_equal", "assert_less_than_equal", "assert_own_property", "assert_regexp_match"]) {
      g[k] = () => {};
    }
  };
})(globalThis);
