window.pdocSearch = (function () {
  /** elasticlunr - http://weixsong.github.io * Copyright (C) 2017 Oliver Nightingale * Copyright (C) 2017 Wei Song * MIT Licensed */ !(function () {
    function e(e) {
      if (null === e || "object" != typeof e) return e;
      var t = e.constructor();
      for (var n in e) e.hasOwnProperty(n) && (t[n] = e[n]);
      return t;
    }
    var t = function (e) {
      var n = new t.Index();
      return (
        n.pipeline.add(t.trimmer, t.stopWordFilter, t.stemmer),
        e && e.call(n, n),
        n
      );
    };
    (t.version = "0.9.5"),
      (lunr = t),
      (t.utils = {}),
      (t.utils.warn = (function (e) {
        return function (t) {
          e.console && console.warn && console.warn(t);
        };
      })(this)),
      (t.utils.toString = function (e) {
        return void 0 === e || null === e ? "" : e.toString();
      }),
      (t.EventEmitter = function () {
        this.events = {};
      }),
      (t.EventEmitter.prototype.addListener = function () {
        var e = Array.prototype.slice.call(arguments),
          t = e.pop(),
          n = e;
        if ("function" != typeof t)
          throw new TypeError("last argument must be a function");
        n.forEach(function (e) {
          this.hasHandler(e) || (this.events[e] = []), this.events[e].push(t);
        }, this);
      }),
      (t.EventEmitter.prototype.removeListener = function (e, t) {
        if (this.hasHandler(e)) {
          var n = this.events[e].indexOf(t);
          -1 !== n &&
            (this.events[e].splice(n, 1),
            0 == this.events[e].length && delete this.events[e]);
        }
      }),
      (t.EventEmitter.prototype.emit = function (e) {
        if (this.hasHandler(e)) {
          var t = Array.prototype.slice.call(arguments, 1);
          this.events[e].forEach(function (e) {
            e.apply(void 0, t);
          }, this);
        }
      }),
      (t.EventEmitter.prototype.hasHandler = function (e) {
        return e in this.events;
      }),
      (t.tokenizer = function (e) {
        if (!arguments.length || null === e || void 0 === e) return [];
        if (Array.isArray(e)) {
          var n = e.filter(function (e) {
            return null === e || void 0 === e ? !1 : !0;
          });
          n = n.map(function (e) {
            return t.utils.toString(e).toLowerCase();
          });
          var i = [];
          return (
            n.forEach(function (e) {
              var n = e.split(t.tokenizer.seperator);
              i = i.concat(n);
            }, this),
            i
          );
        }
        return e.toString().trim().toLowerCase().split(t.tokenizer.seperator);
      }),
      (t.tokenizer.defaultSeperator = /[\s\-]+/),
      (t.tokenizer.seperator = t.tokenizer.defaultSeperator),
      (t.tokenizer.setSeperator = function (e) {
        null !== e &&
          void 0 !== e &&
          "object" == typeof e &&
          (t.tokenizer.seperator = e);
      }),
      (t.tokenizer.resetSeperator = function () {
        t.tokenizer.seperator = t.tokenizer.defaultSeperator;
      }),
      (t.tokenizer.getSeperator = function () {
        return t.tokenizer.seperator;
      }),
      (t.Pipeline = function () {
        this._queue = [];
      }),
      (t.Pipeline.registeredFunctions = {}),
      (t.Pipeline.registerFunction = function (e, n) {
        n in t.Pipeline.registeredFunctions &&
          t.utils.warn("Overwriting existing registered function: " + n),
          (e.label = n),
          (t.Pipeline.registeredFunctions[n] = e);
      }),
      (t.Pipeline.getRegisteredFunction = function (e) {
        return e in t.Pipeline.registeredFunctions != !0
          ? null
          : t.Pipeline.registeredFunctions[e];
      }),
      (t.Pipeline.warnIfFunctionNotRegistered = function (e) {
        var n = e.label && e.label in this.registeredFunctions;
        n ||
          t.utils.warn(
            "Function is not registered with pipeline. This may cause problems when serialising the index.\n",
            e
          );
      }),
      (t.Pipeline.load = function (e) {
        var n = new t.Pipeline();
        return (
          e.forEach(function (e) {
            var i = t.Pipeline.getRegisteredFunction(e);
            if (!i) throw new Error("Cannot load un-registered function: " + e);
            n.add(i);
          }),
          n
        );
      }),
      (t.Pipeline.prototype.add = function () {
        var e = Array.prototype.slice.call(arguments);
        e.forEach(function (e) {
          t.Pipeline.warnIfFunctionNotRegistered(e), this._queue.push(e);
        }, this);
      }),
      (t.Pipeline.prototype.after = function (e, n) {
        t.Pipeline.warnIfFunctionNotRegistered(n);
        var i = this._queue.indexOf(e);
        if (-1 === i) throw new Error("Cannot find existingFn");
        this._queue.splice(i + 1, 0, n);
      }),
      (t.Pipeline.prototype.before = function (e, n) {
        t.Pipeline.warnIfFunctionNotRegistered(n);
        var i = this._queue.indexOf(e);
        if (-1 === i) throw new Error("Cannot find existingFn");
        this._queue.splice(i, 0, n);
      }),
      (t.Pipeline.prototype.remove = function (e) {
        var t = this._queue.indexOf(e);
        -1 !== t && this._queue.splice(t, 1);
      }),
      (t.Pipeline.prototype.run = function (e) {
        for (
          var t = [], n = e.length, i = this._queue.length, o = 0;
          n > o;
          o++
        ) {
          for (
            var r = e[o], s = 0;
            i > s &&
            ((r = this._queue[s](r, o, e)), void 0 !== r && null !== r);
            s++
          );
          void 0 !== r && null !== r && t.push(r);
        }
        return t;
      }),
      (t.Pipeline.prototype.reset = function () {
        this._queue = [];
      }),
      (t.Pipeline.prototype.get = function () {
        return this._queue;
      }),
      (t.Pipeline.prototype.toJSON = function () {
        return this._queue.map(function (e) {
          return t.Pipeline.warnIfFunctionNotRegistered(e), e.label;
        });
      }),
      (t.Index = function () {
        (this._fields = []),
          (this._ref = "id"),
          (this.pipeline = new t.Pipeline()),
          (this.documentStore = new t.DocumentStore()),
          (this.index = {}),
          (this.eventEmitter = new t.EventEmitter()),
          (this._idfCache = {}),
          this.on(
            "add",
            "remove",
            "update",
            function () {
              this._idfCache = {};
            }.bind(this)
          );
      }),
      (t.Index.prototype.on = function () {
        var e = Array.prototype.slice.call(arguments);
        return this.eventEmitter.addListener.apply(this.eventEmitter, e);
      }),
      (t.Index.prototype.off = function (e, t) {
        return this.eventEmitter.removeListener(e, t);
      }),
      (t.Index.load = function (e) {
        e.version !== t.version &&
          t.utils.warn(
            "version mismatch: current " + t.version + " importing " + e.version
          );
        var n = new this();
        (n._fields = e.fields),
          (n._ref = e.ref),
          (n.documentStore = t.DocumentStore.load(e.documentStore)),
          (n.pipeline = t.Pipeline.load(e.pipeline)),
          (n.index = {});
        for (var i in e.index) n.index[i] = t.InvertedIndex.load(e.index[i]);
        return n;
      }),
      (t.Index.prototype.addField = function (e) {
        return (
          this._fields.push(e), (this.index[e] = new t.InvertedIndex()), this
        );
      }),
      (t.Index.prototype.setRef = function (e) {
        return (this._ref = e), this;
      }),
      (t.Index.prototype.saveDocument = function (e) {
        return (this.documentStore = new t.DocumentStore(e)), this;
      }),
      (t.Index.prototype.addDoc = function (e, n) {
        if (e) {
          var n = void 0 === n ? !0 : n,
            i = e[this._ref];
          this.documentStore.addDoc(i, e),
            this._fields.forEach(function (n) {
              var o = this.pipeline.run(t.tokenizer(e[n]));
              this.documentStore.addFieldLength(i, n, o.length);
              var r = {};
              o.forEach(function (e) {
                e in r ? (r[e] += 1) : (r[e] = 1);
              }, this);
              for (var s in r) {
                var u = r[s];
                (u = Math.sqrt(u)),
                  this.index[n].addToken(s, { ref: i, tf: u });
              }
            }, this),
            n && this.eventEmitter.emit("add", e, this);
        }
      }),
      (t.Index.prototype.removeDocByRef = function (e) {
        if (
          e &&
          this.documentStore.isDocStored() !== !1 &&
          this.documentStore.hasDoc(e)
        ) {
          var t = this.documentStore.getDoc(e);
          this.removeDoc(t, !1);
        }
      }),
      (t.Index.prototype.removeDoc = function (e, n) {
        if (e) {
          var n = void 0 === n ? !0 : n,
            i = e[this._ref];
          this.documentStore.hasDoc(i) &&
            (this.documentStore.removeDoc(i),
            this._fields.forEach(function (n) {
              var o = this.pipeline.run(t.tokenizer(e[n]));
              o.forEach(function (e) {
                this.index[n].removeToken(e, i);
              }, this);
            }, this),
            n && this.eventEmitter.emit("remove", e, this));
        }
      }),
      (t.Index.prototype.updateDoc = function (e, t) {
        var t = void 0 === t ? !0 : t;
        this.removeDocByRef(e[this._ref], !1),
          this.addDoc(e, !1),
          t && this.eventEmitter.emit("update", e, this);
      }),
      (t.Index.prototype.idf = function (e, t) {
        var n = "@" + t + "/" + e;
        if (Object.prototype.hasOwnProperty.call(this._idfCache, n))
          return this._idfCache[n];
        var i = this.index[t].getDocFreq(e),
          o = 1 + Math.log(this.documentStore.length / (i + 1));
        return (this._idfCache[n] = o), o;
      }),
      (t.Index.prototype.getFields = function () {
        return this._fields.slice();
      }),
      (t.Index.prototype.search = function (e, n) {
        if (!e) return [];
        e = "string" == typeof e ? { any: e } : JSON.parse(JSON.stringify(e));
        var i = null;
        null != n && (i = JSON.stringify(n));
        for (
          var o = new t.Configuration(i, this.getFields()).get(),
            r = {},
            s = Object.keys(e),
            u = 0;
          u < s.length;
          u++
        ) {
          var a = s[u];
          r[a] = this.pipeline.run(t.tokenizer(e[a]));
        }
        var l = {};
        for (var c in o) {
          var d = r[c] || r.any;
          if (d) {
            var f = this.fieldSearch(d, c, o),
              h = o[c].boost;
            for (var p in f) f[p] = f[p] * h;
            for (var p in f) p in l ? (l[p] += f[p]) : (l[p] = f[p]);
          }
        }
        var v,
          g = [];
        for (var p in l)
          (v = { ref: p, score: l[p] }),
            this.documentStore.hasDoc(p) &&
              (v.doc = this.documentStore.getDoc(p)),
            g.push(v);
        return (
          g.sort(function (e, t) {
            return t.score - e.score;
          }),
          g
        );
      }),
      (t.Index.prototype.fieldSearch = function (e, t, n) {
        var i = n[t].bool,
          o = n[t].expand,
          r = n[t].boost,
          s = null,
          u = {};
        return 0 !== r
          ? (e.forEach(function (e) {
              var n = [e];
              1 == o && (n = this.index[t].expandToken(e));
              var r = {};
              n.forEach(function (n) {
                var o = this.index[t].getDocs(n),
                  a = this.idf(n, t);
                if (s && "AND" == i) {
                  var l = {};
                  for (var c in s) c in o && (l[c] = o[c]);
                  o = l;
                }
                n == e && this.fieldSearchStats(u, n, o);
                for (var c in o) {
                  var d = this.index[t].getTermFrequency(n, c),
                    f = this.documentStore.getFieldLength(c, t),
                    h = 1;
                  0 != f && (h = 1 / Math.sqrt(f));
                  var p = 1;
                  n != e && (p = 0.15 * (1 - (n.length - e.length) / n.length));
                  var v = d * a * h * p;
                  c in r ? (r[c] += v) : (r[c] = v);
                }
              }, this),
                (s = this.mergeScores(s, r, i));
            }, this),
            (s = this.coordNorm(s, u, e.length)))
          : void 0;
      }),
      (t.Index.prototype.mergeScores = function (e, t, n) {
        if (!e) return t;
        if ("AND" == n) {
          var i = {};
          for (var o in t) o in e && (i[o] = e[o] + t[o]);
          return i;
        }
        for (var o in t) o in e ? (e[o] += t[o]) : (e[o] = t[o]);
        return e;
      }),
      (t.Index.prototype.fieldSearchStats = function (e, t, n) {
        for (var i in n) i in e ? e[i].push(t) : (e[i] = [t]);
      }),
      (t.Index.prototype.coordNorm = function (e, t, n) {
        for (var i in e)
          if (i in t) {
            var o = t[i].length;
            e[i] = (e[i] * o) / n;
          }
        return e;
      }),
      (t.Index.prototype.toJSON = function () {
        var e = {};
        return (
          this._fields.forEach(function (t) {
            e[t] = this.index[t].toJSON();
          }, this),
          {
            version: t.version,
            fields: this._fields,
            ref: this._ref,
            documentStore: this.documentStore.toJSON(),
            index: e,
            pipeline: this.pipeline.toJSON(),
          }
        );
      }),
      (t.Index.prototype.use = function (e) {
        var t = Array.prototype.slice.call(arguments, 1);
        t.unshift(this), e.apply(this, t);
      }),
      (t.DocumentStore = function (e) {
        (this._save = null === e || void 0 === e ? !0 : e),
          (this.docs = {}),
          (this.docInfo = {}),
          (this.length = 0);
      }),
      (t.DocumentStore.load = function (e) {
        var t = new this();
        return (
          (t.length = e.length),
          (t.docs = e.docs),
          (t.docInfo = e.docInfo),
          (t._save = e.save),
          t
        );
      }),
      (t.DocumentStore.prototype.isDocStored = function () {
        return this._save;
      }),
      (t.DocumentStore.prototype.addDoc = function (t, n) {
        this.hasDoc(t) || this.length++,
          (this.docs[t] = this._save === !0 ? e(n) : null);
      }),
      (t.DocumentStore.prototype.getDoc = function (e) {
        return this.hasDoc(e) === !1 ? null : this.docs[e];
      }),
      (t.DocumentStore.prototype.hasDoc = function (e) {
        return e in this.docs;
      }),
      (t.DocumentStore.prototype.removeDoc = function (e) {
        this.hasDoc(e) &&
          (delete this.docs[e], delete this.docInfo[e], this.length--);
      }),
      (t.DocumentStore.prototype.addFieldLength = function (e, t, n) {
        null !== e &&
          void 0 !== e &&
          0 != this.hasDoc(e) &&
          (this.docInfo[e] || (this.docInfo[e] = {}), (this.docInfo[e][t] = n));
      }),
      (t.DocumentStore.prototype.updateFieldLength = function (e, t, n) {
        null !== e &&
          void 0 !== e &&
          0 != this.hasDoc(e) &&
          this.addFieldLength(e, t, n);
      }),
      (t.DocumentStore.prototype.getFieldLength = function (e, t) {
        return null === e || void 0 === e
          ? 0
          : e in this.docs && t in this.docInfo[e]
          ? this.docInfo[e][t]
          : 0;
      }),
      (t.DocumentStore.prototype.toJSON = function () {
        return {
          docs: this.docs,
          docInfo: this.docInfo,
          length: this.length,
          save: this._save,
        };
      }),
      (t.stemmer = (function () {
        var e = {
            ational: "ate",
            tional: "tion",
            enci: "ence",
            anci: "ance",
            izer: "ize",
            bli: "ble",
            alli: "al",
            entli: "ent",
            eli: "e",
            ousli: "ous",
            ization: "ize",
            ation: "ate",
            ator: "ate",
            alism: "al",
            iveness: "ive",
            fulness: "ful",
            ousness: "ous",
            aliti: "al",
            iviti: "ive",
            biliti: "ble",
            logi: "log",
          },
          t = {
            icate: "ic",
            ative: "",
            alize: "al",
            iciti: "ic",
            ical: "ic",
            ful: "",
            ness: "",
          },
          n = "[^aeiou]",
          i = "[aeiouy]",
          o = n + "[^aeiouy]*",
          r = i + "[aeiou]*",
          s = "^(" + o + ")?" + r + o,
          u = "^(" + o + ")?" + r + o + "(" + r + ")?$",
          a = "^(" + o + ")?" + r + o + r + o,
          l = "^(" + o + ")?" + i,
          c = new RegExp(s),
          d = new RegExp(a),
          f = new RegExp(u),
          h = new RegExp(l),
          p = /^(.+?)(ss|i)es$/,
          v = /^(.+?)([^s])s$/,
          g = /^(.+?)eed$/,
          m = /^(.+?)(ed|ing)$/,
          y = /.$/,
          S = /(at|bl|iz)$/,
          x = new RegExp("([^aeiouylsz])\\1$"),
          w = new RegExp("^" + o + i + "[^aeiouwxy]$"),
          I = /^(.+?[^aeiou])y$/,
          b =
            /^(.+?)(ational|tional|enci|anci|izer|bli|alli|entli|eli|ousli|ization|ation|ator|alism|iveness|fulness|ousness|aliti|iviti|biliti|logi)$/,
          E = /^(.+?)(icate|ative|alize|iciti|ical|ful|ness)$/,
          D =
            /^(.+?)(al|ance|ence|er|ic|able|ible|ant|ement|ment|ent|ou|ism|ate|iti|ous|ive|ize)$/,
          F = /^(.+?)(s|t)(ion)$/,
          _ = /^(.+?)e$/,
          P = /ll$/,
          k = new RegExp("^" + o + i + "[^aeiouwxy]$"),
          z = function (n) {
            var i, o, r, s, u, a, l;
            if (n.length < 3) return n;
            if (
              ((r = n.substr(0, 1)),
              "y" == r && (n = r.toUpperCase() + n.substr(1)),
              (s = p),
              (u = v),
              s.test(n)
                ? (n = n.replace(s, "$1$2"))
                : u.test(n) && (n = n.replace(u, "$1$2")),
              (s = g),
              (u = m),
              s.test(n))
            ) {
              var z = s.exec(n);
              (s = c), s.test(z[1]) && ((s = y), (n = n.replace(s, "")));
            } else if (u.test(n)) {
              var z = u.exec(n);
              (i = z[1]),
                (u = h),
                u.test(i) &&
                  ((n = i),
                  (u = S),
                  (a = x),
                  (l = w),
                  u.test(n)
                    ? (n += "e")
                    : a.test(n)
                    ? ((s = y), (n = n.replace(s, "")))
                    : l.test(n) && (n += "e"));
            }
            if (((s = I), s.test(n))) {
              var z = s.exec(n);
              (i = z[1]), (n = i + "i");
            }
            if (((s = b), s.test(n))) {
              var z = s.exec(n);
              (i = z[1]), (o = z[2]), (s = c), s.test(i) && (n = i + e[o]);
            }
            if (((s = E), s.test(n))) {
              var z = s.exec(n);
              (i = z[1]), (o = z[2]), (s = c), s.test(i) && (n = i + t[o]);
            }
            if (((s = D), (u = F), s.test(n))) {
              var z = s.exec(n);
              (i = z[1]), (s = d), s.test(i) && (n = i);
            } else if (u.test(n)) {
              var z = u.exec(n);
              (i = z[1] + z[2]), (u = d), u.test(i) && (n = i);
            }
            if (((s = _), s.test(n))) {
              var z = s.exec(n);
              (i = z[1]),
                (s = d),
                (u = f),
                (a = k),
                (s.test(i) || (u.test(i) && !a.test(i))) && (n = i);
            }
            return (
              (s = P),
              (u = d),
              s.test(n) && u.test(n) && ((s = y), (n = n.replace(s, ""))),
              "y" == r && (n = r.toLowerCase() + n.substr(1)),
              n
            );
          };
        return z;
      })()),
      t.Pipeline.registerFunction(t.stemmer, "stemmer"),
      (t.stopWordFilter = function (e) {
        return e && t.stopWordFilter.stopWords[e] !== !0 ? e : void 0;
      }),
      (t.clearStopWords = function () {
        t.stopWordFilter.stopWords = {};
      }),
      (t.addStopWords = function (e) {
        null != e &&
          Array.isArray(e) !== !1 &&
          e.forEach(function (e) {
            t.stopWordFilter.stopWords[e] = !0;
          }, this);
      }),
      (t.resetStopWords = function () {
        t.stopWordFilter.stopWords = t.defaultStopWords;
      }),
      (t.defaultStopWords = {
        "": !0,
        a: !0,
        able: !0,
        about: !0,
        across: !0,
        after: !0,
        all: !0,
        almost: !0,
        also: !0,
        am: !0,
        among: !0,
        an: !0,
        and: !0,
        any: !0,
        are: !0,
        as: !0,
        at: !0,
        be: !0,
        because: !0,
        been: !0,
        but: !0,
        by: !0,
        can: !0,
        cannot: !0,
        could: !0,
        dear: !0,
        did: !0,
        do: !0,
        does: !0,
        either: !0,
        else: !0,
        ever: !0,
        every: !0,
        for: !0,
        from: !0,
        get: !0,
        got: !0,
        had: !0,
        has: !0,
        have: !0,
        he: !0,
        her: !0,
        hers: !0,
        him: !0,
        his: !0,
        how: !0,
        however: !0,
        i: !0,
        if: !0,
        in: !0,
        into: !0,
        is: !0,
        it: !0,
        its: !0,
        just: !0,
        least: !0,
        let: !0,
        like: !0,
        likely: !0,
        may: !0,
        me: !0,
        might: !0,
        most: !0,
        must: !0,
        my: !0,
        neither: !0,
        no: !0,
        nor: !0,
        not: !0,
        of: !0,
        off: !0,
        often: !0,
        on: !0,
        only: !0,
        or: !0,
        other: !0,
        our: !0,
        own: !0,
        rather: !0,
        said: !0,
        say: !0,
        says: !0,
        she: !0,
        should: !0,
        since: !0,
        so: !0,
        some: !0,
        than: !0,
        that: !0,
        the: !0,
        their: !0,
        them: !0,
        then: !0,
        there: !0,
        these: !0,
        they: !0,
        this: !0,
        tis: !0,
        to: !0,
        too: !0,
        twas: !0,
        us: !0,
        wants: !0,
        was: !0,
        we: !0,
        were: !0,
        what: !0,
        when: !0,
        where: !0,
        which: !0,
        while: !0,
        who: !0,
        whom: !0,
        why: !0,
        will: !0,
        with: !0,
        would: !0,
        yet: !0,
        you: !0,
        your: !0,
      }),
      (t.stopWordFilter.stopWords = t.defaultStopWords),
      t.Pipeline.registerFunction(t.stopWordFilter, "stopWordFilter"),
      (t.trimmer = function (e) {
        if (null === e || void 0 === e)
          throw new Error("token should not be undefined");
        return e.replace(/^\W+/, "").replace(/\W+$/, "");
      }),
      t.Pipeline.registerFunction(t.trimmer, "trimmer"),
      (t.InvertedIndex = function () {
        this.root = { docs: {}, df: 0 };
      }),
      (t.InvertedIndex.load = function (e) {
        var t = new this();
        return (t.root = e.root), t;
      }),
      (t.InvertedIndex.prototype.addToken = function (e, t, n) {
        for (var n = n || this.root, i = 0; i <= e.length - 1; ) {
          var o = e[i];
          o in n || (n[o] = { docs: {}, df: 0 }), (i += 1), (n = n[o]);
        }
        var r = t.ref;
        n.docs[r]
          ? (n.docs[r] = { tf: t.tf })
          : ((n.docs[r] = { tf: t.tf }), (n.df += 1));
      }),
      (t.InvertedIndex.prototype.hasToken = function (e) {
        if (!e) return !1;
        for (var t = this.root, n = 0; n < e.length; n++) {
          if (!t[e[n]]) return !1;
          t = t[e[n]];
        }
        return !0;
      }),
      (t.InvertedIndex.prototype.getNode = function (e) {
        if (!e) return null;
        for (var t = this.root, n = 0; n < e.length; n++) {
          if (!t[e[n]]) return null;
          t = t[e[n]];
        }
        return t;
      }),
      (t.InvertedIndex.prototype.getDocs = function (e) {
        var t = this.getNode(e);
        return null == t ? {} : t.docs;
      }),
      (t.InvertedIndex.prototype.getTermFrequency = function (e, t) {
        var n = this.getNode(e);
        return null == n ? 0 : t in n.docs ? n.docs[t].tf : 0;
      }),
      (t.InvertedIndex.prototype.getDocFreq = function (e) {
        var t = this.getNode(e);
        return null == t ? 0 : t.df;
      }),
      (t.InvertedIndex.prototype.removeToken = function (e, t) {
        if (e) {
          var n = this.getNode(e);
          null != n && t in n.docs && (delete n.docs[t], (n.df -= 1));
        }
      }),
      (t.InvertedIndex.prototype.expandToken = function (e, t, n) {
        if (null == e || "" == e) return [];
        var t = t || [];
        if (void 0 == n && ((n = this.getNode(e)), null == n)) return t;
        n.df > 0 && t.push(e);
        for (var i in n)
          "docs" !== i && "df" !== i && this.expandToken(e + i, t, n[i]);
        return t;
      }),
      (t.InvertedIndex.prototype.toJSON = function () {
        return { root: this.root };
      }),
      (t.Configuration = function (e, n) {
        var e = e || "";
        if (void 0 == n || null == n)
          throw new Error("fields should not be null");
        this.config = {};
        var i;
        try {
          (i = JSON.parse(e)), this.buildUserConfig(i, n);
        } catch (o) {
          t.utils.warn(
            "user configuration parse failed, will use default configuration"
          ),
            this.buildDefaultConfig(n);
        }
      }),
      (t.Configuration.prototype.buildDefaultConfig = function (e) {
        this.reset(),
          e.forEach(function (e) {
            this.config[e] = { boost: 1, bool: "OR", expand: !1 };
          }, this);
      }),
      (t.Configuration.prototype.buildUserConfig = function (e, n) {
        var i = "OR",
          o = !1;
        if (
          (this.reset(),
          "bool" in e && (i = e.bool || i),
          "expand" in e && (o = e.expand || o),
          "fields" in e)
        )
          for (var r in e.fields)
            if (n.indexOf(r) > -1) {
              var s = e.fields[r],
                u = o;
              void 0 != s.expand && (u = s.expand),
                (this.config[r] = {
                  boost: s.boost || 0 === s.boost ? s.boost : 1,
                  bool: s.bool || i,
                  expand: u,
                });
            } else
              t.utils.warn(
                "field name in user configuration not found in index instance fields"
              );
        else this.addAllFields2UserConfig(i, o, n);
      }),
      (t.Configuration.prototype.addAllFields2UserConfig = function (e, t, n) {
        n.forEach(function (n) {
          this.config[n] = { boost: 1, bool: e, expand: t };
        }, this);
      }),
      (t.Configuration.prototype.get = function () {
        return this.config;
      }),
      (t.Configuration.prototype.reset = function () {
        this.config = {};
      }),
      (lunr.SortedSet = function () {
        (this.length = 0), (this.elements = []);
      }),
      (lunr.SortedSet.load = function (e) {
        var t = new this();
        return (t.elements = e), (t.length = e.length), t;
      }),
      (lunr.SortedSet.prototype.add = function () {
        var e, t;
        for (e = 0; e < arguments.length; e++)
          (t = arguments[e]),
            ~this.indexOf(t) || this.elements.splice(this.locationFor(t), 0, t);
        this.length = this.elements.length;
      }),
      (lunr.SortedSet.prototype.toArray = function () {
        return this.elements.slice();
      }),
      (lunr.SortedSet.prototype.map = function (e, t) {
        return this.elements.map(e, t);
      }),
      (lunr.SortedSet.prototype.forEach = function (e, t) {
        return this.elements.forEach(e, t);
      }),
      (lunr.SortedSet.prototype.indexOf = function (e) {
        for (
          var t = 0,
            n = this.elements.length,
            i = n - t,
            o = t + Math.floor(i / 2),
            r = this.elements[o];
          i > 1;

        ) {
          if (r === e) return o;
          e > r && (t = o),
            r > e && (n = o),
            (i = n - t),
            (o = t + Math.floor(i / 2)),
            (r = this.elements[o]);
        }
        return r === e ? o : -1;
      }),
      (lunr.SortedSet.prototype.locationFor = function (e) {
        for (
          var t = 0,
            n = this.elements.length,
            i = n - t,
            o = t + Math.floor(i / 2),
            r = this.elements[o];
          i > 1;

        )
          e > r && (t = o),
            r > e && (n = o),
            (i = n - t),
            (o = t + Math.floor(i / 2)),
            (r = this.elements[o]);
        return r > e ? o : e > r ? o + 1 : void 0;
      }),
      (lunr.SortedSet.prototype.intersect = function (e) {
        for (
          var t = new lunr.SortedSet(),
            n = 0,
            i = 0,
            o = this.length,
            r = e.length,
            s = this.elements,
            u = e.elements;
          ;

        ) {
          if (n > o - 1 || i > r - 1) break;
          s[n] !== u[i]
            ? s[n] < u[i]
              ? n++
              : s[n] > u[i] && i++
            : (t.add(s[n]), n++, i++);
        }
        return t;
      }),
      (lunr.SortedSet.prototype.clone = function () {
        var e = new lunr.SortedSet();
        return (e.elements = this.toArray()), (e.length = e.elements.length), e;
      }),
      (lunr.SortedSet.prototype.union = function (e) {
        var t, n, i;
        this.length >= e.length ? ((t = this), (n = e)) : ((t = e), (n = this)),
          (i = t.clone());
        for (var o = 0, r = n.toArray(); o < r.length; o++) i.add(r[o]);
        return i;
      }),
      (lunr.SortedSet.prototype.toJSON = function () {
        return this.toArray();
      }),
      (function (e, t) {
        "function" == typeof define && define.amd
          ? define(t)
          : "object" == typeof exports
          ? (module.exports = t())
          : (e.elasticlunr = t());
      })(this, function () {
        return t;
      });
  })();
  /** pdoc search index */ const docs = {
    version: "0.9.5",
    fields: [
      "qualname",
      "fullname",
      "annotation",
      "default_value",
      "signature",
      "bases",
      "doc",
    ],
    ref: "fullname",
    documentStore: {
      docs: {
        scouter: {
          fullname: "scouter",
          modulename: "scouter",
          kind: "module",
          doc: "<p></p>\n",
        },
        "scouter.Profiler": {
          fullname: "scouter.Profiler",
          modulename: "scouter",
          qualname: "Profiler",
          kind: "class",
          doc: "<p></p>\n",
          bases: "scouter.scouter.ScouterBase",
        },
        "scouter.Profiler.__init__": {
          fullname: "scouter.Profiler.__init__",
          modulename: "scouter",
          qualname: "Profiler.__init__",
          kind: "function",
          doc: "<p>Scouter class for creating data profiles. This class will generate\nbaseline statistics for a given dataset.</p>\n",
          signature: '<span class="signature pdoc-code condensed">()</span>',
        },
        "scouter.Profiler.create_data_profile": {
          fullname: "scouter.Profiler.create_data_profile",
          modulename: "scouter",
          qualname: "Profiler.create_data_profile",
          kind: "function",
          doc: '<p>Create a data profile from data.</p>\n\n<h6 id="arguments">Arguments:</h6>\n\n<ul>\n<li><strong>features:</strong>  Optional list of feature names. If not provided, feature names will be\nautomatically generated.</li>\n<li><strong>data:</strong>  Data to create a data profile from. Data can be a numpy array,\na polars dataframe or pandas dataframe. Data is expected to not contain\nany missing values, NaNs or infinities. These values must be removed or imputed.\nIf NaNs or infinities are present, the data profile will not be created.</li>\n<li><strong>bin_size:</strong>  Optional bin size for histograms. Defaults to 20 bins.</li>\n</ul>\n\n<h6 id="returns">Returns:</h6>\n\n<blockquote>\n  <p>Monitoring profile</p>\n</blockquote>\n',
          signature:
            '<span class="signature pdoc-code multiline">(<span class="param">\t<span class="bp">self</span>,</span><span class="param">\t<span class="n">data</span><span class="p">:</span> <span class="n">Union</span><span class="p">[</span><span class="n">polars</span><span class="o">.</span><span class="n">dataframe</span><span class="o">.</span><span class="n">frame</span><span class="o">.</span><span class="n">DataFrame</span><span class="p">,</span> <span class="n">pandas</span><span class="o">.</span><span class="n">core</span><span class="o">.</span><span class="n">frame</span><span class="o">.</span><span class="n">DataFrame</span><span class="p">,</span> <span class="n">numpy</span><span class="o">.</span><span class="n">ndarray</span><span class="p">[</span><span class="n">Any</span><span class="p">,</span> <span class="n">numpy</span><span class="o">.</span><span class="n">dtype</span><span class="p">[</span><span class="o">+</span><span class="n">_ScalarType_co</span><span class="p">]]]</span>,</span><span class="param">\t<span class="n">features</span><span class="p">:</span> <span class="n">Optional</span><span class="p">[</span><span class="n">List</span><span class="p">[</span><span class="nb">str</span><span class="p">]]</span> <span class="o">=</span> <span class="kc">None</span>,</span><span class="param">\t<span class="n">bin_size</span><span class="p">:</span> <span class="nb">int</span> <span class="o">=</span> <span class="mi">20</span></span><span class="return-annotation">) -> <span class="n">DataProfile</span>:</span></span>',
          funcdef: "def",
        },
        "scouter.Drifter": {
          fullname: "scouter.Drifter",
          modulename: "scouter",
          qualname: "Drifter",
          kind: "class",
          doc: "<p></p>\n",
          bases: "scouter.scouter.ScouterBase",
        },
        "scouter.Drifter.__init__": {
          fullname: "scouter.Drifter.__init__",
          modulename: "scouter",
          qualname: "Drifter.__init__",
          kind: "function",
          doc: "<p>Scouter class for creating monitoring profiles and detecting drift. This class will\ncreate a monitoring profile from a dataset and detect drift from new data. This\nclass is primarily used to setup and actively monitor data drift</p>\n",
          signature: '<span class="signature pdoc-code condensed">()</span>',
        },
        "scouter.Drifter.create_drift_profile": {
          fullname: "scouter.Drifter.create_drift_profile",
          modulename: "scouter",
          qualname: "Drifter.create_drift_profile",
          kind: "function",
          doc: '<p>Create a drift profile from data to use for monitoring.</p>\n\n<h6 id="arguments">Arguments:</h6>\n\n<ul>\n<li><strong>features:</strong>  Optional list of feature names. If not provided, feature names will be\nautomatically generated.</li>\n<li><strong>data:</strong>  Data to create a monitoring profile from. Data can be a numpy array,\na polars dataframe or pandas dataframe. Data is expected to not contain\nany missing values, NaNs or infinities. These values must be removed or imputed.\nIf NaNs or infinities are present, the monitoring profile will not be created.</li>\n<li><strong>monitor_config:</strong>  Configuration for the monitoring profile.</li>\n</ul>\n\n<h6 id="returns">Returns:</h6>\n\n<blockquote>\n  <p>Monitoring profile</p>\n</blockquote>\n',
          signature:
            '<span class="signature pdoc-code multiline">(<span class="param">\t<span class="bp">self</span>,</span><span class="param">\t<span class="n">data</span><span class="p">:</span> <span class="n">Union</span><span class="p">[</span><span class="n">polars</span><span class="o">.</span><span class="n">dataframe</span><span class="o">.</span><span class="n">frame</span><span class="o">.</span><span class="n">DataFrame</span><span class="p">,</span> <span class="n">pandas</span><span class="o">.</span><span class="n">core</span><span class="o">.</span><span class="n">frame</span><span class="o">.</span><span class="n">DataFrame</span><span class="p">,</span> <span class="n">numpy</span><span class="o">.</span><span class="n">ndarray</span><span class="p">[</span><span class="n">Any</span><span class="p">,</span> <span class="n">numpy</span><span class="o">.</span><span class="n">dtype</span><span class="p">[</span><span class="o">+</span><span class="n">_ScalarType_co</span><span class="p">]]]</span>,</span><span class="param">\t<span class="n">monitor_config</span><span class="p">:</span> <span class="n">DriftConfig</span>,</span><span class="param">\t<span class="n">features</span><span class="p">:</span> <span class="n">Optional</span><span class="p">[</span><span class="n">List</span><span class="p">[</span><span class="nb">str</span><span class="p">]]</span> <span class="o">=</span> <span class="kc">None</span></span><span class="return-annotation">) -> <span class="n">DriftProfile</span>:</span></span>',
          funcdef: "def",
        },
        "scouter.Drifter.compute_drift": {
          fullname: "scouter.Drifter.compute_drift",
          modulename: "scouter",
          qualname: "Drifter.compute_drift",
          kind: "function",
          doc: '<p>Compute drift from data and monitoring profile.</p>\n\n<h6 id="arguments">Arguments:</h6>\n\n<ul>\n<li><strong>features:</strong>  Optional list of feature names. If not provided, feature names will be\nautomatically generated. Names must match the feature names in the monitoring profile.</li>\n<li><strong>data:</strong>  Data to compute drift from. Data can be a numpy array,\na polars dataframe or pandas dataframe. Data is expected to not contain\nany missing values, NaNs or infinities.</li>\n<li><strong>drift_profile:</strong>  Monitoring profile containing feature drift profiles.</li>\n</ul>\n',
          signature:
            '<span class="signature pdoc-code multiline">(<span class="param">\t<span class="bp">self</span>,</span><span class="param">\t<span class="n">data</span><span class="p">:</span> <span class="n">Union</span><span class="p">[</span><span class="n">polars</span><span class="o">.</span><span class="n">dataframe</span><span class="o">.</span><span class="n">frame</span><span class="o">.</span><span class="n">DataFrame</span><span class="p">,</span> <span class="n">pandas</span><span class="o">.</span><span class="n">core</span><span class="o">.</span><span class="n">frame</span><span class="o">.</span><span class="n">DataFrame</span><span class="p">,</span> <span class="n">numpy</span><span class="o">.</span><span class="n">ndarray</span><span class="p">[</span><span class="n">Any</span><span class="p">,</span> <span class="n">numpy</span><span class="o">.</span><span class="n">dtype</span><span class="p">[</span><span class="o">+</span><span class="n">_ScalarType_co</span><span class="p">]]]</span>,</span><span class="param">\t<span class="n">drift_profile</span><span class="p">:</span> <span class="n">DriftProfile</span>,</span><span class="param">\t<span class="n">features</span><span class="p">:</span> <span class="n">Optional</span><span class="p">[</span><span class="n">List</span><span class="p">[</span><span class="nb">str</span><span class="p">]]</span> <span class="o">=</span> <span class="kc">None</span></span><span class="return-annotation">) -> <span class="n">DriftMap</span>:</span></span>',
          funcdef: "def",
        },
        "scouter.Drifter.generate_alerts": {
          fullname: "scouter.Drifter.generate_alerts",
          modulename: "scouter",
          qualname: "Drifter.generate_alerts",
          kind: "function",
          doc: '<p>Generate alerts from a drift array and features.</p>\n\n<h6 id="arguments">Arguments:</h6>\n\n<ul>\n<li><strong>drift_array:</strong>  Array of drift values.</li>\n<li><strong>features:</strong>  List of feature names. Must match the order of the drift array.</li>\n<li><strong>alert_rule:</strong>  Alert rule to apply to drift values.</li>\n</ul>\n\n<h6 id="returns">Returns:</h6>\n\n<blockquote>\n  <p>Dictionary of alerts.</p>\n</blockquote>\n',
          signature:
            '<span class="signature pdoc-code multiline">(<span class="param">\t<span class="bp">self</span>,</span><span class="param">\t<span class="n">drift_array</span><span class="p">:</span> <span class="n">numpy</span><span class="o">.</span><span class="n">ndarray</span><span class="p">[</span><span class="n">typing</span><span class="o">.</span><span class="n">Any</span><span class="p">,</span> <span class="n">numpy</span><span class="o">.</span><span class="n">dtype</span><span class="p">[</span><span class="o">+</span><span class="n">_ScalarType_co</span><span class="p">]]</span>,</span><span class="param">\t<span class="n">features</span><span class="p">:</span> <span class="n">List</span><span class="p">[</span><span class="nb">str</span><span class="p">]</span>,</span><span class="param">\t<span class="n">alert_rule</span><span class="p">:</span> <span class="n">AlertRule</span></span><span class="return-annotation">) -> <span class="n">FeatureAlerts</span>:</span></span>',
          funcdef: "def",
        },
        "scouter.DataProfile": {
          fullname: "scouter.DataProfile",
          modulename: "scouter",
          qualname: "DataProfile",
          kind: "class",
          doc: "<p></p>\n",
        },
        "scouter.DataProfile.model_dump_json": {
          fullname: "scouter.DataProfile.model_dump_json",
          modulename: "scouter",
          qualname: "DataProfile.model_dump_json",
          kind: "function",
          doc: "<p></p>\n",
          signature:
            '<span class="signature pdoc-code condensed">(<span class="param"><span class="bp">self</span>, </span><span class="param"><span class="o">/</span></span><span class="return-annotation">):</span></span>',
          funcdef: "def",
        },
        "scouter.DataProfile.load_from_json": {
          fullname: "scouter.DataProfile.load_from_json",
          modulename: "scouter",
          qualname: "DataProfile.load_from_json",
          kind: "function",
          doc: "<p></p>\n",
          signature:
            '<span class="signature pdoc-code condensed">(<span class="param"><span class="n">model</span></span><span class="return-annotation">):</span></span>',
          funcdef: "def",
        },
        "scouter.DataProfile.save_to_json": {
          fullname: "scouter.DataProfile.save_to_json",
          modulename: "scouter",
          qualname: "DataProfile.save_to_json",
          kind: "function",
          doc: "<p></p>\n",
          signature:
            '<span class="signature pdoc-code condensed">(<span class="param"><span class="bp">self</span>, </span><span class="param"><span class="o">/</span>, </span><span class="param"><span class="n">path</span><span class="o">=</span><span class="kc">None</span></span><span class="return-annotation">):</span></span>',
          funcdef: "def",
        },
        "scouter.DataProfile.features": {
          fullname: "scouter.DataProfile.features",
          modulename: "scouter",
          qualname: "DataProfile.features",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.DriftProfile": {
          fullname: "scouter.DriftProfile",
          modulename: "scouter",
          qualname: "DriftProfile",
          kind: "class",
          doc: "<p></p>\n",
        },
        "scouter.DriftProfile.model_dump_json": {
          fullname: "scouter.DriftProfile.model_dump_json",
          modulename: "scouter",
          qualname: "DriftProfile.model_dump_json",
          kind: "function",
          doc: "<p></p>\n",
          signature:
            '<span class="signature pdoc-code condensed">(<span class="param"><span class="bp">self</span>, </span><span class="param"><span class="o">/</span></span><span class="return-annotation">):</span></span>',
          funcdef: "def",
        },
        "scouter.DriftProfile.load_from_json": {
          fullname: "scouter.DriftProfile.load_from_json",
          modulename: "scouter",
          qualname: "DriftProfile.load_from_json",
          kind: "function",
          doc: "<p></p>\n",
          signature:
            '<span class="signature pdoc-code condensed">(<span class="param"><span class="n">model</span></span><span class="return-annotation">):</span></span>',
          funcdef: "def",
        },
        "scouter.DriftProfile.save_to_json": {
          fullname: "scouter.DriftProfile.save_to_json",
          modulename: "scouter",
          qualname: "DriftProfile.save_to_json",
          kind: "function",
          doc: "<p></p>\n",
          signature:
            '<span class="signature pdoc-code condensed">(<span class="param"><span class="bp">self</span>, </span><span class="param"><span class="o">/</span>, </span><span class="param"><span class="n">path</span><span class="o">=</span><span class="kc">None</span></span><span class="return-annotation">):</span></span>',
          funcdef: "def",
        },
        "scouter.DriftProfile.features": {
          fullname: "scouter.DriftProfile.features",
          modulename: "scouter",
          qualname: "DriftProfile.features",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.DriftProfile.config": {
          fullname: "scouter.DriftProfile.config",
          modulename: "scouter",
          qualname: "DriftProfile.config",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.FeatureDriftProfile": {
          fullname: "scouter.FeatureDriftProfile",
          modulename: "scouter",
          qualname: "FeatureDriftProfile",
          kind: "class",
          doc: '<p>Python class for a monitoring profile</p>\n\n<h1 id="arguments">Arguments</h1>\n\n<ul>\n<li><code>id</code> - The id value</li>\n<li><code>center</code> - The center value</li>\n<li><code>ucl</code> - The upper control limit</li>\n<li><code>lcl</code> - The lower control limit</li>\n<li><code>timestamp</code> - The timestamp value</li>\n</ul>\n',
        },
        "scouter.FeatureDriftProfile.two_lcl": {
          fullname: "scouter.FeatureDriftProfile.two_lcl",
          modulename: "scouter",
          qualname: "FeatureDriftProfile.two_lcl",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.FeatureDriftProfile.two_ucl": {
          fullname: "scouter.FeatureDriftProfile.two_ucl",
          modulename: "scouter",
          qualname: "FeatureDriftProfile.two_ucl",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.FeatureDriftProfile.one_ucl": {
          fullname: "scouter.FeatureDriftProfile.one_ucl",
          modulename: "scouter",
          qualname: "FeatureDriftProfile.one_ucl",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.FeatureDriftProfile.three_ucl": {
          fullname: "scouter.FeatureDriftProfile.three_ucl",
          modulename: "scouter",
          qualname: "FeatureDriftProfile.three_ucl",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.FeatureDriftProfile.center": {
          fullname: "scouter.FeatureDriftProfile.center",
          modulename: "scouter",
          qualname: "FeatureDriftProfile.center",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.FeatureDriftProfile.id": {
          fullname: "scouter.FeatureDriftProfile.id",
          modulename: "scouter",
          qualname: "FeatureDriftProfile.id",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.FeatureDriftProfile.one_lcl": {
          fullname: "scouter.FeatureDriftProfile.one_lcl",
          modulename: "scouter",
          qualname: "FeatureDriftProfile.one_lcl",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.FeatureDriftProfile.three_lcl": {
          fullname: "scouter.FeatureDriftProfile.three_lcl",
          modulename: "scouter",
          qualname: "FeatureDriftProfile.three_lcl",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.FeatureDriftProfile.timestamp": {
          fullname: "scouter.FeatureDriftProfile.timestamp",
          modulename: "scouter",
          qualname: "FeatureDriftProfile.timestamp",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.FeatureProfile": {
          fullname: "scouter.FeatureProfile",
          modulename: "scouter",
          qualname: "FeatureProfile",
          kind: "class",
          doc: "<p></p>\n",
        },
        "scouter.FeatureProfile.quantiles": {
          fullname: "scouter.FeatureProfile.quantiles",
          modulename: "scouter",
          qualname: "FeatureProfile.quantiles",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.FeatureProfile.distinct": {
          fullname: "scouter.FeatureProfile.distinct",
          modulename: "scouter",
          qualname: "FeatureProfile.distinct",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.FeatureProfile.stddev": {
          fullname: "scouter.FeatureProfile.stddev",
          modulename: "scouter",
          qualname: "FeatureProfile.stddev",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.FeatureProfile.timestamp": {
          fullname: "scouter.FeatureProfile.timestamp",
          modulename: "scouter",
          qualname: "FeatureProfile.timestamp",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.FeatureProfile.histogram": {
          fullname: "scouter.FeatureProfile.histogram",
          modulename: "scouter",
          qualname: "FeatureProfile.histogram",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.FeatureProfile.id": {
          fullname: "scouter.FeatureProfile.id",
          modulename: "scouter",
          qualname: "FeatureProfile.id",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.FeatureProfile.mean": {
          fullname: "scouter.FeatureProfile.mean",
          modulename: "scouter",
          qualname: "FeatureProfile.mean",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.FeatureProfile.max": {
          fullname: "scouter.FeatureProfile.max",
          modulename: "scouter",
          qualname: "FeatureProfile.max",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.FeatureProfile.min": {
          fullname: "scouter.FeatureProfile.min",
          modulename: "scouter",
          qualname: "FeatureProfile.min",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.Alert": {
          fullname: "scouter.Alert",
          modulename: "scouter",
          qualname: "Alert",
          kind: "class",
          doc: "<p></p>\n",
        },
        "scouter.Alert.kind": {
          fullname: "scouter.Alert.kind",
          modulename: "scouter",
          qualname: "Alert.kind",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.Alert.zone": {
          fullname: "scouter.Alert.zone",
          modulename: "scouter",
          qualname: "Alert.zone",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.AlertType": {
          fullname: "scouter.AlertType",
          modulename: "scouter",
          qualname: "AlertType",
          kind: "class",
          doc: "<p>str(object='') -> str\nstr(bytes_or_buffer[, encoding[, errors]]) -> str</p>\n\n<p>Create a new string object from the given object. If encoding or\nerrors is specified, then the object must expose a data buffer\nthat will be decoded using the given encoding and error handler.\nOtherwise, returns the result of object.__str__() (if defined)\nor repr(object).\nencoding defaults to sys.getdefaultencoding().\nerrors defaults to 'strict'.</p>\n",
          bases: "builtins.str, enum.Enum",
        },
        "scouter.AlertType.OutOfBounds": {
          fullname: "scouter.AlertType.OutOfBounds",
          modulename: "scouter",
          qualname: "AlertType.OutOfBounds",
          kind: "variable",
          doc: "<p></p>\n",
          default_value:
            "&lt;AlertType.OutOfBounds: &#x27;Out of Bounds&#x27;&gt;",
        },
        "scouter.AlertType.Consecutive": {
          fullname: "scouter.AlertType.Consecutive",
          modulename: "scouter",
          qualname: "AlertType.Consecutive",
          kind: "variable",
          doc: "<p></p>\n",
          default_value:
            "&lt;AlertType.Consecutive: &#x27;Consecutive&#x27;&gt;",
        },
        "scouter.AlertType.Alternating": {
          fullname: "scouter.AlertType.Alternating",
          modulename: "scouter",
          qualname: "AlertType.Alternating",
          kind: "variable",
          doc: "<p></p>\n",
          default_value:
            "&lt;AlertType.Alternating: &#x27;Alternating&#x27;&gt;",
        },
        "scouter.AlertType.AllGood": {
          fullname: "scouter.AlertType.AllGood",
          modulename: "scouter",
          qualname: "AlertType.AllGood",
          kind: "variable",
          doc: "<p></p>\n",
          default_value: "&lt;AlertType.AllGood: &#x27;All Good&#x27;&gt;",
        },
        "scouter.AlertType.Trend": {
          fullname: "scouter.AlertType.Trend",
          modulename: "scouter",
          qualname: "AlertType.Trend",
          kind: "variable",
          doc: "<p></p>\n",
          default_value: "&lt;AlertType.Trend: &#x27;Trend&#x27;&gt;",
        },
        "scouter.AlertRule": {
          fullname: "scouter.AlertRule",
          modulename: "scouter",
          qualname: "AlertRule",
          kind: "class",
          doc: "<p></p>\n",
        },
        "scouter.AlertRule.to_str": {
          fullname: "scouter.AlertRule.to_str",
          modulename: "scouter",
          qualname: "AlertRule.to_str",
          kind: "function",
          doc: "<p></p>\n",
          signature:
            '<span class="signature pdoc-code condensed">(<span class="param"><span class="bp">self</span>, </span><span class="param"><span class="o">/</span></span><span class="return-annotation">):</span></span>',
          funcdef: "def",
        },
        "scouter.AlertRule.process": {
          fullname: "scouter.AlertRule.process",
          modulename: "scouter",
          qualname: "AlertRule.process",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.AlertRule.percentage": {
          fullname: "scouter.AlertRule.percentage",
          modulename: "scouter",
          qualname: "AlertRule.percentage",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.AlertZone": {
          fullname: "scouter.AlertZone",
          modulename: "scouter",
          qualname: "AlertZone",
          kind: "class",
          doc: "<p>str(object='') -> str\nstr(bytes_or_buffer[, encoding[, errors]]) -> str</p>\n\n<p>Create a new string object from the given object. If encoding or\nerrors is specified, then the object must expose a data buffer\nthat will be decoded using the given encoding and error handler.\nOtherwise, returns the result of object.__str__() (if defined)\nor repr(object).\nencoding defaults to sys.getdefaultencoding().\nerrors defaults to 'strict'.</p>\n",
          bases: "builtins.str, enum.Enum",
        },
        "scouter.AlertZone.Zone1": {
          fullname: "scouter.AlertZone.Zone1",
          modulename: "scouter",
          qualname: "AlertZone.Zone1",
          kind: "variable",
          doc: "<p></p>\n",
          default_value: "&lt;AlertZone.Zone1: &#x27;Zone 1&#x27;&gt;",
        },
        "scouter.AlertZone.Zone2": {
          fullname: "scouter.AlertZone.Zone2",
          modulename: "scouter",
          qualname: "AlertZone.Zone2",
          kind: "variable",
          doc: "<p></p>\n",
          default_value: "&lt;AlertZone.Zone2: &#x27;Zone 2&#x27;&gt;",
        },
        "scouter.AlertZone.Zone3": {
          fullname: "scouter.AlertZone.Zone3",
          modulename: "scouter",
          qualname: "AlertZone.Zone3",
          kind: "variable",
          doc: "<p></p>\n",
          default_value: "&lt;AlertZone.Zone3: &#x27;Zone 3&#x27;&gt;",
        },
        "scouter.AlertZone.OutOfBounds": {
          fullname: "scouter.AlertZone.OutOfBounds",
          modulename: "scouter",
          qualname: "AlertZone.OutOfBounds",
          kind: "variable",
          doc: "<p></p>\n",
          default_value:
            "&lt;AlertZone.OutOfBounds: &#x27;Out of Bounds&#x27;&gt;",
        },
        "scouter.AlertZone.NotApplicable": {
          fullname: "scouter.AlertZone.NotApplicable",
          modulename: "scouter",
          qualname: "AlertZone.NotApplicable",
          kind: "variable",
          doc: "<p></p>\n",
          default_value: "&lt;AlertZone.NotApplicable: &#x27;NA&#x27;&gt;",
        },
        "scouter.FeatureAlerts": {
          fullname: "scouter.FeatureAlerts",
          modulename: "scouter",
          qualname: "FeatureAlerts",
          kind: "class",
          doc: "<p></p>\n",
        },
        "scouter.FeatureAlerts.model_dump_json": {
          fullname: "scouter.FeatureAlerts.model_dump_json",
          modulename: "scouter",
          qualname: "FeatureAlerts.model_dump_json",
          kind: "function",
          doc: "<p></p>\n",
          signature:
            '<span class="signature pdoc-code condensed">(<span class="param"><span class="bp">self</span>, </span><span class="param"><span class="o">/</span></span><span class="return-annotation">):</span></span>',
          funcdef: "def",
        },
        "scouter.FeatureAlerts.features": {
          fullname: "scouter.FeatureAlerts.features",
          modulename: "scouter",
          qualname: "FeatureAlerts.features",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.ProcessAlertRule": {
          fullname: "scouter.ProcessAlertRule",
          modulename: "scouter",
          qualname: "ProcessAlertRule",
          kind: "class",
          doc: "<p></p>\n",
        },
        "scouter.ProcessAlertRule.rule": {
          fullname: "scouter.ProcessAlertRule.rule",
          modulename: "scouter",
          qualname: "ProcessAlertRule.rule",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.PercentageAlertRule": {
          fullname: "scouter.PercentageAlertRule",
          modulename: "scouter",
          qualname: "PercentageAlertRule",
          kind: "class",
          doc: "<p></p>\n",
        },
        "scouter.PercentageAlertRule.rule": {
          fullname: "scouter.PercentageAlertRule.rule",
          modulename: "scouter",
          qualname: "PercentageAlertRule.rule",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.CommonCrons": {
          fullname: "scouter.CommonCrons",
          modulename: "scouter",
          qualname: "CommonCrons",
          kind: "variable",
          doc: "<p></p>\n",
          default_value: "&lt;builtins.CommonCron object&gt;",
        },
        "scouter.CommonCron": {
          fullname: "scouter.CommonCron",
          modulename: "scouter",
          qualname: "CommonCron",
          kind: "class",
          doc: "<p></p>\n",
        },
        "scouter.CommonCron.every_day": {
          fullname: "scouter.CommonCron.every_day",
          modulename: "scouter",
          qualname: "CommonCron.every_day",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.CommonCron.every_week": {
          fullname: "scouter.CommonCron.every_week",
          modulename: "scouter",
          qualname: "CommonCron.every_week",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.CommonCron.every_hour": {
          fullname: "scouter.CommonCron.every_hour",
          modulename: "scouter",
          qualname: "CommonCron.every_hour",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.CommonCron.every_6_hours": {
          fullname: "scouter.CommonCron.every_6_hours",
          modulename: "scouter",
          qualname: "CommonCron.every_6_hours",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.CommonCron.every_30_minutes": {
          fullname: "scouter.CommonCron.every_30_minutes",
          modulename: "scouter",
          qualname: "CommonCron.every_30_minutes",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.CommonCron.every_12_hours": {
          fullname: "scouter.CommonCron.every_12_hours",
          modulename: "scouter",
          qualname: "CommonCron.every_12_hours",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.Every30Minutes": {
          fullname: "scouter.Every30Minutes",
          modulename: "scouter",
          qualname: "Every30Minutes",
          kind: "class",
          doc: "<p></p>\n",
        },
        "scouter.Every30Minutes.get_next": {
          fullname: "scouter.Every30Minutes.get_next",
          modulename: "scouter",
          qualname: "Every30Minutes.get_next",
          kind: "function",
          doc: "<p></p>\n",
          signature:
            '<span class="signature pdoc-code condensed">(<span class="param"><span class="bp">self</span>, </span><span class="param"><span class="o">/</span></span><span class="return-annotation">):</span></span>',
          funcdef: "def",
        },
        "scouter.Every30Minutes.cron": {
          fullname: "scouter.Every30Minutes.cron",
          modulename: "scouter",
          qualname: "Every30Minutes.cron",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.EveryHour": {
          fullname: "scouter.EveryHour",
          modulename: "scouter",
          qualname: "EveryHour",
          kind: "class",
          doc: "<p></p>\n",
        },
        "scouter.EveryHour.get_next": {
          fullname: "scouter.EveryHour.get_next",
          modulename: "scouter",
          qualname: "EveryHour.get_next",
          kind: "function",
          doc: "<p></p>\n",
          signature:
            '<span class="signature pdoc-code condensed">(<span class="param"><span class="bp">self</span>, </span><span class="param"><span class="o">/</span></span><span class="return-annotation">):</span></span>',
          funcdef: "def",
        },
        "scouter.EveryHour.cron": {
          fullname: "scouter.EveryHour.cron",
          modulename: "scouter",
          qualname: "EveryHour.cron",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.Every6Hours": {
          fullname: "scouter.Every6Hours",
          modulename: "scouter",
          qualname: "Every6Hours",
          kind: "class",
          doc: "<p></p>\n",
        },
        "scouter.Every6Hours.get_next": {
          fullname: "scouter.Every6Hours.get_next",
          modulename: "scouter",
          qualname: "Every6Hours.get_next",
          kind: "function",
          doc: "<p></p>\n",
          signature:
            '<span class="signature pdoc-code condensed">(<span class="param"><span class="bp">self</span>, </span><span class="param"><span class="o">/</span></span><span class="return-annotation">):</span></span>',
          funcdef: "def",
        },
        "scouter.Every6Hours.cron": {
          fullname: "scouter.Every6Hours.cron",
          modulename: "scouter",
          qualname: "Every6Hours.cron",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.Every12Hours": {
          fullname: "scouter.Every12Hours",
          modulename: "scouter",
          qualname: "Every12Hours",
          kind: "class",
          doc: "<p></p>\n",
        },
        "scouter.Every12Hours.get_next": {
          fullname: "scouter.Every12Hours.get_next",
          modulename: "scouter",
          qualname: "Every12Hours.get_next",
          kind: "function",
          doc: "<p></p>\n",
          signature:
            '<span class="signature pdoc-code condensed">(<span class="param"><span class="bp">self</span>, </span><span class="param"><span class="o">/</span></span><span class="return-annotation">):</span></span>',
          funcdef: "def",
        },
        "scouter.Every12Hours.cron": {
          fullname: "scouter.Every12Hours.cron",
          modulename: "scouter",
          qualname: "Every12Hours.cron",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.EveryDay": {
          fullname: "scouter.EveryDay",
          modulename: "scouter",
          qualname: "EveryDay",
          kind: "class",
          doc: "<p></p>\n",
        },
        "scouter.EveryDay.get_next": {
          fullname: "scouter.EveryDay.get_next",
          modulename: "scouter",
          qualname: "EveryDay.get_next",
          kind: "function",
          doc: "<p></p>\n",
          signature:
            '<span class="signature pdoc-code condensed">(<span class="param"><span class="bp">self</span>, </span><span class="param"><span class="o">/</span></span><span class="return-annotation">):</span></span>',
          funcdef: "def",
        },
        "scouter.EveryDay.cron": {
          fullname: "scouter.EveryDay.cron",
          modulename: "scouter",
          qualname: "EveryDay.cron",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.EveryWeek": {
          fullname: "scouter.EveryWeek",
          modulename: "scouter",
          qualname: "EveryWeek",
          kind: "class",
          doc: "<p></p>\n",
        },
        "scouter.EveryWeek.get_next": {
          fullname: "scouter.EveryWeek.get_next",
          modulename: "scouter",
          qualname: "EveryWeek.get_next",
          kind: "function",
          doc: "<p></p>\n",
          signature:
            '<span class="signature pdoc-code condensed">(<span class="param"><span class="bp">self</span>, </span><span class="param"><span class="o">/</span></span><span class="return-annotation">):</span></span>',
          funcdef: "def",
        },
        "scouter.EveryWeek.cron": {
          fullname: "scouter.EveryWeek.cron",
          modulename: "scouter",
          qualname: "EveryWeek.cron",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.DriftConfig": {
          fullname: "scouter.DriftConfig",
          modulename: "scouter",
          qualname: "DriftConfig",
          kind: "class",
          doc: '<p>Python class for a monitoring configuration</p>\n\n<h1 id="arguments">Arguments</h1>\n\n<ul>\n<li><code>sample_size</code> - The sample size</li>\n<li><code>sample</code> - Whether to sample data or not, Default is true</li>\n<li><code>name</code> - The name of the model</li>\n<li><code>repository</code> - The repository associated with the model</li>\n<li><code>version</code> - The version of the model</li>\n<li><code>schedule</code> - The cron schedule for monitoring</li>\n<li><code>alert_rule</code> - The alerting rule to use for monitoring</li>\n</ul>\n',
        },
        "scouter.DriftConfig.sample_size": {
          fullname: "scouter.DriftConfig.sample_size",
          modulename: "scouter",
          qualname: "DriftConfig.sample_size",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.DriftConfig.name": {
          fullname: "scouter.DriftConfig.name",
          modulename: "scouter",
          qualname: "DriftConfig.name",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.DriftConfig.sample": {
          fullname: "scouter.DriftConfig.sample",
          modulename: "scouter",
          qualname: "DriftConfig.sample",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.DriftConfig.version": {
          fullname: "scouter.DriftConfig.version",
          modulename: "scouter",
          qualname: "DriftConfig.version",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.DriftConfig.schedule": {
          fullname: "scouter.DriftConfig.schedule",
          modulename: "scouter",
          qualname: "DriftConfig.schedule",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.DriftConfig.repository": {
          fullname: "scouter.DriftConfig.repository",
          modulename: "scouter",
          qualname: "DriftConfig.repository",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.DriftConfig.alert_rule": {
          fullname: "scouter.DriftConfig.alert_rule",
          modulename: "scouter",
          qualname: "DriftConfig.alert_rule",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.DriftMap": {
          fullname: "scouter.DriftMap",
          modulename: "scouter",
          qualname: "DriftMap",
          kind: "class",
          doc: '<p>Python class for a Drift map of features with calculated drift</p>\n\n<h1 id="arguments">Arguments</h1>\n\n<ul>\n<li><code>features</code> - A hashmap of feature names and their drift</li>\n</ul>\n',
        },
        "scouter.DriftMap.add_feature": {
          fullname: "scouter.DriftMap.add_feature",
          modulename: "scouter",
          qualname: "DriftMap.add_feature",
          kind: "function",
          doc: "<p></p>\n",
          signature:
            '<span class="signature pdoc-code condensed">(<span class="param"><span class="bp">self</span>, </span><span class="param"><span class="o">/</span>, </span><span class="param"><span class="n">feature</span>, </span><span class="param"><span class="n">profile</span></span><span class="return-annotation">):</span></span>',
          funcdef: "def",
        },
        "scouter.DriftMap.model_dump_json": {
          fullname: "scouter.DriftMap.model_dump_json",
          modulename: "scouter",
          qualname: "DriftMap.model_dump_json",
          kind: "function",
          doc: "<p></p>\n",
          signature:
            '<span class="signature pdoc-code condensed">(<span class="param"><span class="bp">self</span>, </span><span class="param"><span class="o">/</span></span><span class="return-annotation">):</span></span>',
          funcdef: "def",
        },
        "scouter.DriftMap.load_from_json": {
          fullname: "scouter.DriftMap.load_from_json",
          modulename: "scouter",
          qualname: "DriftMap.load_from_json",
          kind: "function",
          doc: "<p></p>\n",
          signature:
            '<span class="signature pdoc-code condensed">(<span class="param"><span class="n">model</span></span><span class="return-annotation">):</span></span>',
          funcdef: "def",
        },
        "scouter.DriftMap.save_to_json": {
          fullname: "scouter.DriftMap.save_to_json",
          modulename: "scouter",
          qualname: "DriftMap.save_to_json",
          kind: "function",
          doc: "<p></p>\n",
          signature:
            '<span class="signature pdoc-code condensed">(<span class="param"><span class="bp">self</span>, </span><span class="param"><span class="o">/</span>, </span><span class="param"><span class="n">path</span><span class="o">=</span><span class="kc">None</span></span><span class="return-annotation">):</span></span>',
          funcdef: "def",
        },
        "scouter.DriftMap.to_server_record": {
          fullname: "scouter.DriftMap.to_server_record",
          modulename: "scouter",
          qualname: "DriftMap.to_server_record",
          kind: "function",
          doc: "<p></p>\n",
          signature:
            '<span class="signature pdoc-code condensed">(<span class="param"><span class="bp">self</span>, </span><span class="param"><span class="o">/</span></span><span class="return-annotation">):</span></span>',
          funcdef: "def",
        },
        "scouter.DriftMap.to_numpy": {
          fullname: "scouter.DriftMap.to_numpy",
          modulename: "scouter",
          qualname: "DriftMap.to_numpy",
          kind: "function",
          doc: "<p></p>\n",
          signature:
            '<span class="signature pdoc-code condensed">(<span class="param"><span class="bp">self</span>, </span><span class="param"><span class="o">/</span></span><span class="return-annotation">):</span></span>',
          funcdef: "def",
        },
        "scouter.DriftMap.name": {
          fullname: "scouter.DriftMap.name",
          modulename: "scouter",
          qualname: "DriftMap.name",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.DriftMap.repository": {
          fullname: "scouter.DriftMap.repository",
          modulename: "scouter",
          qualname: "DriftMap.repository",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.DriftMap.version": {
          fullname: "scouter.DriftMap.version",
          modulename: "scouter",
          qualname: "DriftMap.version",
          kind: "variable",
          doc: "<p></p>\n",
        },
        "scouter.DriftMap.features": {
          fullname: "scouter.DriftMap.features",
          modulename: "scouter",
          qualname: "DriftMap.features",
          kind: "variable",
          doc: "<p></p>\n",
        },
      },
      docInfo: {
        scouter: {
          qualname: 0,
          fullname: 1,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.Profiler": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 3,
          doc: 3,
        },
        "scouter.Profiler.__init__": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 4,
          bases: 0,
          doc: 19,
        },
        "scouter.Profiler.create_data_profile": {
          qualname: 4,
          fullname: 5,
          annotation: 0,
          default_value: 0,
          signature: 149,
          bases: 0,
          doc: 120,
        },
        "scouter.Drifter": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 3,
          doc: 3,
        },
        "scouter.Drifter.__init__": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 4,
          bases: 0,
          doc: 39,
        },
        "scouter.Drifter.create_drift_profile": {
          qualname: 4,
          fullname: 5,
          annotation: 0,
          default_value: 0,
          signature: 142,
          bases: 0,
          doc: 120,
        },
        "scouter.Drifter.compute_drift": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 142,
          bases: 0,
          doc: 95,
        },
        "scouter.Drifter.generate_alerts": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 91,
          bases: 0,
          doc: 74,
        },
        "scouter.DataProfile": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.DataProfile.model_dump_json": {
          qualname: 4,
          fullname: 5,
          annotation: 0,
          default_value: 0,
          signature: 16,
          bases: 0,
          doc: 3,
        },
        "scouter.DataProfile.load_from_json": {
          qualname: 4,
          fullname: 5,
          annotation: 0,
          default_value: 0,
          signature: 11,
          bases: 0,
          doc: 3,
        },
        "scouter.DataProfile.save_to_json": {
          qualname: 4,
          fullname: 5,
          annotation: 0,
          default_value: 0,
          signature: 26,
          bases: 0,
          doc: 3,
        },
        "scouter.DataProfile.features": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftProfile": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftProfile.model_dump_json": {
          qualname: 4,
          fullname: 5,
          annotation: 0,
          default_value: 0,
          signature: 16,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftProfile.load_from_json": {
          qualname: 4,
          fullname: 5,
          annotation: 0,
          default_value: 0,
          signature: 11,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftProfile.save_to_json": {
          qualname: 4,
          fullname: 5,
          annotation: 0,
          default_value: 0,
          signature: 26,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftProfile.features": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftProfile.config": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureDriftProfile": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 57,
        },
        "scouter.FeatureDriftProfile.two_lcl": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureDriftProfile.two_ucl": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureDriftProfile.one_ucl": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureDriftProfile.three_ucl": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureDriftProfile.center": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureDriftProfile.id": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureDriftProfile.one_lcl": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureDriftProfile.three_lcl": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureDriftProfile.timestamp": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureProfile": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureProfile.quantiles": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureProfile.distinct": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureProfile.stddev": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureProfile.timestamp": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureProfile.histogram": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureProfile.id": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureProfile.mean": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureProfile.max": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureProfile.min": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.Alert": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.Alert.kind": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.Alert.zone": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.AlertType": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 4,
          doc: 72,
        },
        "scouter.AlertType.OutOfBounds": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 11,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.AlertType.Consecutive": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 9,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.AlertType.Alternating": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 9,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.AlertType.AllGood": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 10,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.AlertType.Trend": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 9,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.AlertRule": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.AlertRule.to_str": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 16,
          bases: 0,
          doc: 3,
        },
        "scouter.AlertRule.process": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.AlertRule.percentage": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.AlertZone": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 4,
          doc: 72,
        },
        "scouter.AlertZone.Zone1": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 10,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.AlertZone.Zone2": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 10,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.AlertZone.Zone3": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 10,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.AlertZone.OutOfBounds": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 11,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.AlertZone.NotApplicable": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 9,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureAlerts": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureAlerts.model_dump_json": {
          qualname: 4,
          fullname: 5,
          annotation: 0,
          default_value: 0,
          signature: 16,
          bases: 0,
          doc: 3,
        },
        "scouter.FeatureAlerts.features": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.ProcessAlertRule": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.ProcessAlertRule.rule": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.PercentageAlertRule": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.PercentageAlertRule.rule": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.CommonCrons": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 7,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.CommonCron": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.CommonCron.every_day": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.CommonCron.every_week": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.CommonCron.every_hour": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.CommonCron.every_6_hours": {
          qualname: 4,
          fullname: 5,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.CommonCron.every_30_minutes": {
          qualname: 4,
          fullname: 5,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.CommonCron.every_12_hours": {
          qualname: 4,
          fullname: 5,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.Every30Minutes": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.Every30Minutes.get_next": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 16,
          bases: 0,
          doc: 3,
        },
        "scouter.Every30Minutes.cron": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.EveryHour": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.EveryHour.get_next": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 16,
          bases: 0,
          doc: 3,
        },
        "scouter.EveryHour.cron": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.Every6Hours": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.Every6Hours.get_next": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 16,
          bases: 0,
          doc: 3,
        },
        "scouter.Every6Hours.cron": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.Every12Hours": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.Every12Hours.get_next": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 16,
          bases: 0,
          doc: 3,
        },
        "scouter.Every12Hours.cron": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.EveryDay": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.EveryDay.get_next": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 16,
          bases: 0,
          doc: 3,
        },
        "scouter.EveryDay.cron": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.EveryWeek": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.EveryWeek.get_next": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 16,
          bases: 0,
          doc: 3,
        },
        "scouter.EveryWeek.cron": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftConfig": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 92,
        },
        "scouter.DriftConfig.sample_size": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftConfig.name": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftConfig.sample": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftConfig.version": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftConfig.schedule": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftConfig.repository": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftConfig.alert_rule": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftMap": {
          qualname: 1,
          fullname: 2,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 33,
        },
        "scouter.DriftMap.add_feature": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 26,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftMap.model_dump_json": {
          qualname: 4,
          fullname: 5,
          annotation: 0,
          default_value: 0,
          signature: 16,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftMap.load_from_json": {
          qualname: 4,
          fullname: 5,
          annotation: 0,
          default_value: 0,
          signature: 11,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftMap.save_to_json": {
          qualname: 4,
          fullname: 5,
          annotation: 0,
          default_value: 0,
          signature: 26,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftMap.to_server_record": {
          qualname: 4,
          fullname: 5,
          annotation: 0,
          default_value: 0,
          signature: 16,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftMap.to_numpy": {
          qualname: 3,
          fullname: 4,
          annotation: 0,
          default_value: 0,
          signature: 16,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftMap.name": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftMap.repository": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftMap.version": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
        "scouter.DriftMap.features": {
          qualname: 2,
          fullname: 3,
          annotation: 0,
          default_value: 0,
          signature: 0,
          bases: 0,
          doc: 3,
        },
      },
      length: 111,
      save: true,
    },
    index: {
      qualname: {
        root: {
          1: {
            2: {
              docs: { "scouter.CommonCron.every_12_hours": { tf: 1 } },
              df: 1,
            },
            docs: {},
            df: 0,
          },
          3: {
            0: {
              docs: { "scouter.CommonCron.every_30_minutes": { tf: 1 } },
              df: 1,
            },
            docs: {},
            df: 0,
          },
          6: { docs: { "scouter.CommonCron.every_6_hours": { tf: 1 } }, df: 1 },
          docs: {
            "scouter.Profiler.__init__": { tf: 1 },
            "scouter.Drifter.__init__": { tf: 1 },
          },
          df: 2,
          p: {
            docs: {},
            df: 0,
            r: {
              docs: {},
              df: 0,
              o: {
                docs: {},
                df: 0,
                f: {
                  docs: {},
                  df: 0,
                  i: {
                    docs: {},
                    df: 0,
                    l: {
                      docs: {},
                      df: 0,
                      e: {
                        docs: {
                          "scouter.Profiler.create_data_profile": { tf: 1 },
                          "scouter.Drifter.create_drift_profile": { tf: 1 },
                        },
                        df: 2,
                        r: {
                          docs: {
                            "scouter.Profiler": { tf: 1 },
                            "scouter.Profiler.__init__": { tf: 1 },
                            "scouter.Profiler.create_data_profile": { tf: 1 },
                          },
                          df: 3,
                        },
                      },
                    },
                  },
                },
                c: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {},
                    df: 0,
                    s: {
                      docs: {},
                      df: 0,
                      s: {
                        docs: { "scouter.AlertRule.process": { tf: 1 } },
                        df: 1,
                        a: {
                          docs: {},
                          df: 0,
                          l: {
                            docs: {},
                            df: 0,
                            e: {
                              docs: {},
                              df: 0,
                              r: {
                                docs: {},
                                df: 0,
                                t: {
                                  docs: {},
                                  df: 0,
                                  r: {
                                    docs: {},
                                    df: 0,
                                    u: {
                                      docs: {},
                                      df: 0,
                                      l: {
                                        docs: {},
                                        df: 0,
                                        e: {
                                          docs: {
                                            "scouter.ProcessAlertRule": {
                                              tf: 1,
                                            },
                                            "scouter.ProcessAlertRule.rule": {
                                              tf: 1,
                                            },
                                          },
                                          df: 2,
                                        },
                                      },
                                    },
                                  },
                                },
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              r: {
                docs: {},
                df: 0,
                c: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {},
                    df: 0,
                    n: {
                      docs: {},
                      df: 0,
                      t: {
                        docs: {},
                        df: 0,
                        a: {
                          docs: {},
                          df: 0,
                          g: {
                            docs: {},
                            df: 0,
                            e: {
                              docs: {
                                "scouter.AlertRule.percentage": { tf: 1 },
                              },
                              df: 1,
                              a: {
                                docs: {},
                                df: 0,
                                l: {
                                  docs: {},
                                  df: 0,
                                  e: {
                                    docs: {},
                                    df: 0,
                                    r: {
                                      docs: {},
                                      df: 0,
                                      t: {
                                        docs: {},
                                        df: 0,
                                        r: {
                                          docs: {},
                                          df: 0,
                                          u: {
                                            docs: {},
                                            df: 0,
                                            l: {
                                              docs: {},
                                              df: 0,
                                              e: {
                                                docs: {
                                                  "scouter.PercentageAlertRule":
                                                    { tf: 1 },
                                                  "scouter.PercentageAlertRule.rule":
                                                    { tf: 1 },
                                                },
                                                df: 2,
                                              },
                                            },
                                          },
                                        },
                                      },
                                    },
                                  },
                                },
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          i: {
            docs: {},
            df: 0,
            n: {
              docs: {},
              df: 0,
              i: {
                docs: {},
                df: 0,
                t: {
                  docs: {
                    "scouter.Profiler.__init__": { tf: 1 },
                    "scouter.Drifter.__init__": { tf: 1 },
                  },
                  df: 2,
                },
              },
            },
            d: {
              docs: {
                "scouter.FeatureDriftProfile.id": { tf: 1 },
                "scouter.FeatureProfile.id": { tf: 1 },
              },
              df: 2,
            },
          },
          c: {
            docs: {},
            df: 0,
            r: {
              docs: {},
              df: 0,
              e: {
                docs: {},
                df: 0,
                a: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {},
                    df: 0,
                    e: {
                      docs: {
                        "scouter.Profiler.create_data_profile": { tf: 1 },
                        "scouter.Drifter.create_drift_profile": { tf: 1 },
                      },
                      df: 2,
                    },
                  },
                },
              },
              o: {
                docs: {},
                df: 0,
                n: {
                  docs: {
                    "scouter.Every30Minutes.cron": { tf: 1 },
                    "scouter.EveryHour.cron": { tf: 1 },
                    "scouter.Every6Hours.cron": { tf: 1 },
                    "scouter.Every12Hours.cron": { tf: 1 },
                    "scouter.EveryDay.cron": { tf: 1 },
                    "scouter.EveryWeek.cron": { tf: 1 },
                  },
                  df: 6,
                },
              },
            },
            o: {
              docs: {},
              df: 0,
              m: {
                docs: {},
                df: 0,
                p: {
                  docs: {},
                  df: 0,
                  u: {
                    docs: {},
                    df: 0,
                    t: {
                      docs: {},
                      df: 0,
                      e: {
                        docs: { "scouter.Drifter.compute_drift": { tf: 1 } },
                        df: 1,
                      },
                    },
                  },
                },
                m: {
                  docs: {},
                  df: 0,
                  o: {
                    docs: {},
                    df: 0,
                    n: {
                      docs: {},
                      df: 0,
                      c: {
                        docs: {},
                        df: 0,
                        r: {
                          docs: {},
                          df: 0,
                          o: {
                            docs: {},
                            df: 0,
                            n: {
                              docs: {
                                "scouter.CommonCron": { tf: 1 },
                                "scouter.CommonCron.every_day": { tf: 1 },
                                "scouter.CommonCron.every_week": { tf: 1 },
                                "scouter.CommonCron.every_hour": { tf: 1 },
                                "scouter.CommonCron.every_6_hours": { tf: 1 },
                                "scouter.CommonCron.every_30_minutes": {
                                  tf: 1,
                                },
                                "scouter.CommonCron.every_12_hours": { tf: 1 },
                              },
                              df: 7,
                              s: {
                                docs: { "scouter.CommonCrons": { tf: 1 } },
                                df: 1,
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
              n: {
                docs: {},
                df: 0,
                f: {
                  docs: {},
                  df: 0,
                  i: {
                    docs: {},
                    df: 0,
                    g: {
                      docs: { "scouter.DriftProfile.config": { tf: 1 } },
                      df: 1,
                    },
                  },
                },
                s: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {},
                    df: 0,
                    c: {
                      docs: {},
                      df: 0,
                      u: {
                        docs: {},
                        df: 0,
                        t: {
                          docs: {},
                          df: 0,
                          i: {
                            docs: {},
                            df: 0,
                            v: {
                              docs: {},
                              df: 0,
                              e: {
                                docs: {
                                  "scouter.AlertType.Consecutive": { tf: 1 },
                                },
                                df: 1,
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              n: {
                docs: {},
                df: 0,
                t: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {},
                    df: 0,
                    r: {
                      docs: { "scouter.FeatureDriftProfile.center": { tf: 1 } },
                      df: 1,
                    },
                  },
                },
              },
            },
          },
          d: {
            docs: {},
            df: 0,
            a: {
              docs: {},
              df: 0,
              t: {
                docs: {},
                df: 0,
                a: {
                  docs: { "scouter.Profiler.create_data_profile": { tf: 1 } },
                  df: 1,
                  p: {
                    docs: {},
                    df: 0,
                    r: {
                      docs: {},
                      df: 0,
                      o: {
                        docs: {},
                        df: 0,
                        f: {
                          docs: {},
                          df: 0,
                          i: {
                            docs: {},
                            df: 0,
                            l: {
                              docs: {},
                              df: 0,
                              e: {
                                docs: {
                                  "scouter.DataProfile": { tf: 1 },
                                  "scouter.DataProfile.model_dump_json": {
                                    tf: 1,
                                  },
                                  "scouter.DataProfile.load_from_json": {
                                    tf: 1,
                                  },
                                  "scouter.DataProfile.save_to_json": { tf: 1 },
                                  "scouter.DataProfile.features": { tf: 1 },
                                },
                                df: 5,
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
              y: { docs: { "scouter.CommonCron.every_day": { tf: 1 } }, df: 1 },
            },
            r: {
              docs: {},
              df: 0,
              i: {
                docs: {},
                df: 0,
                f: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {
                      "scouter.Drifter.create_drift_profile": { tf: 1 },
                      "scouter.Drifter.compute_drift": { tf: 1 },
                    },
                    df: 2,
                    e: {
                      docs: {},
                      df: 0,
                      r: {
                        docs: {
                          "scouter.Drifter": { tf: 1 },
                          "scouter.Drifter.__init__": { tf: 1 },
                          "scouter.Drifter.create_drift_profile": { tf: 1 },
                          "scouter.Drifter.compute_drift": { tf: 1 },
                          "scouter.Drifter.generate_alerts": { tf: 1 },
                        },
                        df: 5,
                      },
                    },
                    p: {
                      docs: {},
                      df: 0,
                      r: {
                        docs: {},
                        df: 0,
                        o: {
                          docs: {},
                          df: 0,
                          f: {
                            docs: {},
                            df: 0,
                            i: {
                              docs: {},
                              df: 0,
                              l: {
                                docs: {},
                                df: 0,
                                e: {
                                  docs: {
                                    "scouter.DriftProfile": { tf: 1 },
                                    "scouter.DriftProfile.model_dump_json": {
                                      tf: 1,
                                    },
                                    "scouter.DriftProfile.load_from_json": {
                                      tf: 1,
                                    },
                                    "scouter.DriftProfile.save_to_json": {
                                      tf: 1,
                                    },
                                    "scouter.DriftProfile.features": { tf: 1 },
                                    "scouter.DriftProfile.config": { tf: 1 },
                                  },
                                  df: 6,
                                },
                              },
                            },
                          },
                        },
                      },
                    },
                    c: {
                      docs: {},
                      df: 0,
                      o: {
                        docs: {},
                        df: 0,
                        n: {
                          docs: {},
                          df: 0,
                          f: {
                            docs: {},
                            df: 0,
                            i: {
                              docs: {},
                              df: 0,
                              g: {
                                docs: {
                                  "scouter.DriftConfig": { tf: 1 },
                                  "scouter.DriftConfig.sample_size": { tf: 1 },
                                  "scouter.DriftConfig.name": { tf: 1 },
                                  "scouter.DriftConfig.sample": { tf: 1 },
                                  "scouter.DriftConfig.version": { tf: 1 },
                                  "scouter.DriftConfig.schedule": { tf: 1 },
                                  "scouter.DriftConfig.repository": { tf: 1 },
                                  "scouter.DriftConfig.alert_rule": { tf: 1 },
                                },
                                df: 8,
                              },
                            },
                          },
                        },
                      },
                    },
                    m: {
                      docs: {},
                      df: 0,
                      a: {
                        docs: {},
                        df: 0,
                        p: {
                          docs: {
                            "scouter.DriftMap": { tf: 1 },
                            "scouter.DriftMap.add_feature": { tf: 1 },
                            "scouter.DriftMap.model_dump_json": { tf: 1 },
                            "scouter.DriftMap.load_from_json": { tf: 1 },
                            "scouter.DriftMap.save_to_json": { tf: 1 },
                            "scouter.DriftMap.to_server_record": { tf: 1 },
                            "scouter.DriftMap.to_numpy": { tf: 1 },
                            "scouter.DriftMap.name": { tf: 1 },
                            "scouter.DriftMap.repository": { tf: 1 },
                            "scouter.DriftMap.version": { tf: 1 },
                            "scouter.DriftMap.features": { tf: 1 },
                          },
                          df: 11,
                        },
                      },
                    },
                  },
                },
              },
            },
            u: {
              docs: {},
              df: 0,
              m: {
                docs: {},
                df: 0,
                p: {
                  docs: {
                    "scouter.DataProfile.model_dump_json": { tf: 1 },
                    "scouter.DriftProfile.model_dump_json": { tf: 1 },
                    "scouter.FeatureAlerts.model_dump_json": { tf: 1 },
                    "scouter.DriftMap.model_dump_json": { tf: 1 },
                  },
                  df: 4,
                },
              },
            },
            i: {
              docs: {},
              df: 0,
              s: {
                docs: {},
                df: 0,
                t: {
                  docs: {},
                  df: 0,
                  i: {
                    docs: {},
                    df: 0,
                    n: {
                      docs: {},
                      df: 0,
                      c: {
                        docs: {},
                        df: 0,
                        t: {
                          docs: {
                            "scouter.FeatureProfile.distinct": { tf: 1 },
                          },
                          df: 1,
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          g: {
            docs: {},
            df: 0,
            e: {
              docs: {},
              df: 0,
              n: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  r: {
                    docs: {},
                    df: 0,
                    a: {
                      docs: {},
                      df: 0,
                      t: {
                        docs: {},
                        df: 0,
                        e: {
                          docs: {
                            "scouter.Drifter.generate_alerts": { tf: 1 },
                          },
                          df: 1,
                        },
                      },
                    },
                  },
                },
              },
              t: {
                docs: {
                  "scouter.Every30Minutes.get_next": { tf: 1 },
                  "scouter.EveryHour.get_next": { tf: 1 },
                  "scouter.Every6Hours.get_next": { tf: 1 },
                  "scouter.Every12Hours.get_next": { tf: 1 },
                  "scouter.EveryDay.get_next": { tf: 1 },
                  "scouter.EveryWeek.get_next": { tf: 1 },
                },
                df: 6,
              },
            },
          },
          a: {
            docs: {},
            df: 0,
            l: {
              docs: {},
              df: 0,
              e: {
                docs: {},
                df: 0,
                r: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {
                      "scouter.Alert": { tf: 1 },
                      "scouter.Alert.kind": { tf: 1 },
                      "scouter.Alert.zone": { tf: 1 },
                      "scouter.DriftConfig.alert_rule": { tf: 1 },
                    },
                    df: 4,
                    s: {
                      docs: { "scouter.Drifter.generate_alerts": { tf: 1 } },
                      df: 1,
                    },
                    t: {
                      docs: {},
                      df: 0,
                      y: {
                        docs: {},
                        df: 0,
                        p: {
                          docs: {},
                          df: 0,
                          e: {
                            docs: {
                              "scouter.AlertType": { tf: 1 },
                              "scouter.AlertType.OutOfBounds": { tf: 1 },
                              "scouter.AlertType.Consecutive": { tf: 1 },
                              "scouter.AlertType.Alternating": { tf: 1 },
                              "scouter.AlertType.AllGood": { tf: 1 },
                              "scouter.AlertType.Trend": { tf: 1 },
                            },
                            df: 6,
                          },
                        },
                      },
                    },
                    r: {
                      docs: {},
                      df: 0,
                      u: {
                        docs: {},
                        df: 0,
                        l: {
                          docs: {},
                          df: 0,
                          e: {
                            docs: {
                              "scouter.AlertRule": { tf: 1 },
                              "scouter.AlertRule.to_str": { tf: 1 },
                              "scouter.AlertRule.process": { tf: 1 },
                              "scouter.AlertRule.percentage": { tf: 1 },
                            },
                            df: 4,
                          },
                        },
                      },
                    },
                    z: {
                      docs: {},
                      df: 0,
                      o: {
                        docs: {},
                        df: 0,
                        n: {
                          docs: {},
                          df: 0,
                          e: {
                            docs: {
                              "scouter.AlertZone": { tf: 1 },
                              "scouter.AlertZone.Zone1": { tf: 1 },
                              "scouter.AlertZone.Zone2": { tf: 1 },
                              "scouter.AlertZone.Zone3": { tf: 1 },
                              "scouter.AlertZone.OutOfBounds": { tf: 1 },
                              "scouter.AlertZone.NotApplicable": { tf: 1 },
                            },
                            df: 6,
                          },
                        },
                      },
                    },
                  },
                },
              },
              t: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  r: {
                    docs: {},
                    df: 0,
                    n: {
                      docs: {},
                      df: 0,
                      a: {
                        docs: {},
                        df: 0,
                        t: {
                          docs: {},
                          df: 0,
                          i: {
                            docs: {},
                            df: 0,
                            n: {
                              docs: {},
                              df: 0,
                              g: {
                                docs: {
                                  "scouter.AlertType.Alternating": { tf: 1 },
                                },
                                df: 1,
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
              l: {
                docs: {},
                df: 0,
                g: {
                  docs: {},
                  df: 0,
                  o: {
                    docs: {},
                    df: 0,
                    o: {
                      docs: {},
                      df: 0,
                      d: {
                        docs: { "scouter.AlertType.AllGood": { tf: 1 } },
                        df: 1,
                      },
                    },
                  },
                },
              },
            },
            d: {
              docs: {},
              df: 0,
              d: { docs: { "scouter.DriftMap.add_feature": { tf: 1 } }, df: 1 },
            },
          },
          m: {
            docs: {},
            df: 0,
            o: {
              docs: {},
              df: 0,
              d: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  l: {
                    docs: {
                      "scouter.DataProfile.model_dump_json": { tf: 1 },
                      "scouter.DriftProfile.model_dump_json": { tf: 1 },
                      "scouter.FeatureAlerts.model_dump_json": { tf: 1 },
                      "scouter.DriftMap.model_dump_json": { tf: 1 },
                    },
                    df: 4,
                  },
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              a: {
                docs: {},
                df: 0,
                n: {
                  docs: { "scouter.FeatureProfile.mean": { tf: 1 } },
                  df: 1,
                },
              },
            },
            a: {
              docs: {},
              df: 0,
              x: { docs: { "scouter.FeatureProfile.max": { tf: 1 } }, df: 1 },
            },
            i: {
              docs: {},
              df: 0,
              n: {
                docs: { "scouter.FeatureProfile.min": { tf: 1 } },
                df: 1,
                u: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {},
                    df: 0,
                    e: {
                      docs: {},
                      df: 0,
                      s: {
                        docs: {
                          "scouter.CommonCron.every_30_minutes": { tf: 1 },
                        },
                        df: 1,
                      },
                    },
                  },
                },
              },
            },
          },
          j: {
            docs: {},
            df: 0,
            s: {
              docs: {},
              df: 0,
              o: {
                docs: {},
                df: 0,
                n: {
                  docs: {
                    "scouter.DataProfile.model_dump_json": { tf: 1 },
                    "scouter.DataProfile.load_from_json": { tf: 1 },
                    "scouter.DataProfile.save_to_json": { tf: 1 },
                    "scouter.DriftProfile.model_dump_json": { tf: 1 },
                    "scouter.DriftProfile.load_from_json": { tf: 1 },
                    "scouter.DriftProfile.save_to_json": { tf: 1 },
                    "scouter.FeatureAlerts.model_dump_json": { tf: 1 },
                    "scouter.DriftMap.model_dump_json": { tf: 1 },
                    "scouter.DriftMap.load_from_json": { tf: 1 },
                    "scouter.DriftMap.save_to_json": { tf: 1 },
                  },
                  df: 10,
                },
              },
            },
          },
          l: {
            docs: {},
            df: 0,
            o: {
              docs: {},
              df: 0,
              a: {
                docs: {},
                df: 0,
                d: {
                  docs: {
                    "scouter.DataProfile.load_from_json": { tf: 1 },
                    "scouter.DriftProfile.load_from_json": { tf: 1 },
                    "scouter.DriftMap.load_from_json": { tf: 1 },
                  },
                  df: 3,
                },
              },
            },
            c: {
              docs: {},
              df: 0,
              l: {
                docs: {
                  "scouter.FeatureDriftProfile.two_lcl": { tf: 1 },
                  "scouter.FeatureDriftProfile.one_lcl": { tf: 1 },
                  "scouter.FeatureDriftProfile.three_lcl": { tf: 1 },
                },
                df: 3,
              },
            },
          },
          f: {
            docs: {},
            df: 0,
            r: {
              docs: {},
              df: 0,
              o: {
                docs: {},
                df: 0,
                m: {
                  docs: {
                    "scouter.DataProfile.load_from_json": { tf: 1 },
                    "scouter.DriftProfile.load_from_json": { tf: 1 },
                    "scouter.DriftMap.load_from_json": { tf: 1 },
                  },
                  df: 3,
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              a: {
                docs: {},
                df: 0,
                t: {
                  docs: {},
                  df: 0,
                  u: {
                    docs: {},
                    df: 0,
                    r: {
                      docs: {},
                      df: 0,
                      e: {
                        docs: { "scouter.DriftMap.add_feature": { tf: 1 } },
                        df: 1,
                        s: {
                          docs: {
                            "scouter.DataProfile.features": { tf: 1 },
                            "scouter.DriftProfile.features": { tf: 1 },
                            "scouter.FeatureAlerts.features": { tf: 1 },
                            "scouter.DriftMap.features": { tf: 1 },
                          },
                          df: 4,
                        },
                        d: {
                          docs: {},
                          df: 0,
                          r: {
                            docs: {},
                            df: 0,
                            i: {
                              docs: {},
                              df: 0,
                              f: {
                                docs: {},
                                df: 0,
                                t: {
                                  docs: {},
                                  df: 0,
                                  p: {
                                    docs: {},
                                    df: 0,
                                    r: {
                                      docs: {},
                                      df: 0,
                                      o: {
                                        docs: {},
                                        df: 0,
                                        f: {
                                          docs: {},
                                          df: 0,
                                          i: {
                                            docs: {},
                                            df: 0,
                                            l: {
                                              docs: {},
                                              df: 0,
                                              e: {
                                                docs: {
                                                  "scouter.FeatureDriftProfile":
                                                    { tf: 1 },
                                                  "scouter.FeatureDriftProfile.two_lcl":
                                                    { tf: 1 },
                                                  "scouter.FeatureDriftProfile.two_ucl":
                                                    { tf: 1 },
                                                  "scouter.FeatureDriftProfile.one_ucl":
                                                    { tf: 1 },
                                                  "scouter.FeatureDriftProfile.three_ucl":
                                                    { tf: 1 },
                                                  "scouter.FeatureDriftProfile.center":
                                                    { tf: 1 },
                                                  "scouter.FeatureDriftProfile.id":
                                                    { tf: 1 },
                                                  "scouter.FeatureDriftProfile.one_lcl":
                                                    { tf: 1 },
                                                  "scouter.FeatureDriftProfile.three_lcl":
                                                    { tf: 1 },
                                                  "scouter.FeatureDriftProfile.timestamp":
                                                    { tf: 1 },
                                                },
                                                df: 10,
                                              },
                                            },
                                          },
                                        },
                                      },
                                    },
                                  },
                                },
                              },
                            },
                          },
                          a: {
                            docs: {},
                            df: 0,
                            t: {
                              docs: {},
                              df: 0,
                              a: {
                                docs: {},
                                df: 0,
                                p: {
                                  docs: {},
                                  df: 0,
                                  r: {
                                    docs: {},
                                    df: 0,
                                    o: {
                                      docs: {},
                                      df: 0,
                                      f: {
                                        docs: {},
                                        df: 0,
                                        i: {
                                          docs: {},
                                          df: 0,
                                          l: {
                                            docs: {},
                                            df: 0,
                                            e: {
                                              docs: {
                                                "scouter.FeatureProfile": {
                                                  tf: 1,
                                                },
                                                "scouter.FeatureProfile.quantiles":
                                                  { tf: 1 },
                                                "scouter.FeatureProfile.distinct":
                                                  { tf: 1 },
                                                "scouter.FeatureProfile.stddev":
                                                  { tf: 1 },
                                                "scouter.FeatureProfile.timestamp":
                                                  { tf: 1 },
                                                "scouter.FeatureProfile.histogram":
                                                  { tf: 1 },
                                                "scouter.FeatureProfile.id": {
                                                  tf: 1,
                                                },
                                                "scouter.FeatureProfile.mean": {
                                                  tf: 1,
                                                },
                                                "scouter.FeatureProfile.max": {
                                                  tf: 1,
                                                },
                                                "scouter.FeatureProfile.min": {
                                                  tf: 1,
                                                },
                                              },
                                              df: 10,
                                            },
                                          },
                                        },
                                      },
                                    },
                                  },
                                },
                              },
                            },
                          },
                        },
                        a: {
                          docs: {},
                          df: 0,
                          l: {
                            docs: {},
                            df: 0,
                            e: {
                              docs: {},
                              df: 0,
                              r: {
                                docs: {},
                                df: 0,
                                t: {
                                  docs: {},
                                  df: 0,
                                  s: {
                                    docs: {
                                      "scouter.FeatureAlerts": { tf: 1 },
                                      "scouter.FeatureAlerts.model_dump_json": {
                                        tf: 1,
                                      },
                                      "scouter.FeatureAlerts.features": {
                                        tf: 1,
                                      },
                                    },
                                    df: 3,
                                  },
                                },
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          s: {
            docs: {},
            df: 0,
            a: {
              docs: {},
              df: 0,
              v: {
                docs: {},
                df: 0,
                e: {
                  docs: {
                    "scouter.DataProfile.save_to_json": { tf: 1 },
                    "scouter.DriftProfile.save_to_json": { tf: 1 },
                    "scouter.DriftMap.save_to_json": { tf: 1 },
                  },
                  df: 3,
                },
              },
              m: {
                docs: {},
                df: 0,
                p: {
                  docs: {},
                  df: 0,
                  l: {
                    docs: {},
                    df: 0,
                    e: {
                      docs: {
                        "scouter.DriftConfig.sample_size": { tf: 1 },
                        "scouter.DriftConfig.sample": { tf: 1 },
                      },
                      df: 2,
                    },
                  },
                },
              },
            },
            t: {
              docs: {},
              df: 0,
              d: {
                docs: {},
                df: 0,
                d: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {},
                    df: 0,
                    v: {
                      docs: { "scouter.FeatureProfile.stddev": { tf: 1 } },
                      df: 1,
                    },
                  },
                },
              },
              r: { docs: { "scouter.AlertRule.to_str": { tf: 1 } }, df: 1 },
            },
            i: {
              docs: {},
              df: 0,
              z: {
                docs: {},
                df: 0,
                e: {
                  docs: { "scouter.DriftConfig.sample_size": { tf: 1 } },
                  df: 1,
                },
              },
            },
            c: {
              docs: {},
              df: 0,
              h: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  d: {
                    docs: {},
                    df: 0,
                    u: {
                      docs: {},
                      df: 0,
                      l: {
                        docs: {},
                        df: 0,
                        e: {
                          docs: { "scouter.DriftConfig.schedule": { tf: 1 } },
                          df: 1,
                        },
                      },
                    },
                  },
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              r: {
                docs: {},
                df: 0,
                v: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {},
                    df: 0,
                    r: {
                      docs: { "scouter.DriftMap.to_server_record": { tf: 1 } },
                      df: 1,
                    },
                  },
                },
              },
            },
          },
          t: {
            docs: {},
            df: 0,
            o: {
              docs: {
                "scouter.DataProfile.save_to_json": { tf: 1 },
                "scouter.DriftProfile.save_to_json": { tf: 1 },
                "scouter.AlertRule.to_str": { tf: 1 },
                "scouter.DriftMap.save_to_json": { tf: 1 },
                "scouter.DriftMap.to_server_record": { tf: 1 },
                "scouter.DriftMap.to_numpy": { tf: 1 },
              },
              df: 6,
            },
            w: {
              docs: {},
              df: 0,
              o: {
                docs: {
                  "scouter.FeatureDriftProfile.two_lcl": { tf: 1 },
                  "scouter.FeatureDriftProfile.two_ucl": { tf: 1 },
                },
                df: 2,
              },
            },
            h: {
              docs: {},
              df: 0,
              r: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {
                      "scouter.FeatureDriftProfile.three_ucl": { tf: 1 },
                      "scouter.FeatureDriftProfile.three_lcl": { tf: 1 },
                    },
                    df: 2,
                  },
                },
              },
            },
            i: {
              docs: {},
              df: 0,
              m: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  s: {
                    docs: {},
                    df: 0,
                    t: {
                      docs: {},
                      df: 0,
                      a: {
                        docs: {},
                        df: 0,
                        m: {
                          docs: {},
                          df: 0,
                          p: {
                            docs: {
                              "scouter.FeatureDriftProfile.timestamp": {
                                tf: 1,
                              },
                              "scouter.FeatureProfile.timestamp": { tf: 1 },
                            },
                            df: 2,
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            r: {
              docs: {},
              df: 0,
              e: {
                docs: {},
                df: 0,
                n: {
                  docs: {},
                  df: 0,
                  d: { docs: { "scouter.AlertType.Trend": { tf: 1 } }, df: 1 },
                },
              },
            },
          },
          u: {
            docs: {},
            df: 0,
            c: {
              docs: {},
              df: 0,
              l: {
                docs: {
                  "scouter.FeatureDriftProfile.two_ucl": { tf: 1 },
                  "scouter.FeatureDriftProfile.one_ucl": { tf: 1 },
                  "scouter.FeatureDriftProfile.three_ucl": { tf: 1 },
                },
                df: 3,
              },
            },
          },
          o: {
            docs: {},
            df: 0,
            n: {
              docs: {},
              df: 0,
              e: {
                docs: {
                  "scouter.FeatureDriftProfile.one_ucl": { tf: 1 },
                  "scouter.FeatureDriftProfile.one_lcl": { tf: 1 },
                },
                df: 2,
              },
            },
            u: {
              docs: {},
              df: 0,
              t: {
                docs: {},
                df: 0,
                o: {
                  docs: {},
                  df: 0,
                  f: {
                    docs: {},
                    df: 0,
                    b: {
                      docs: {},
                      df: 0,
                      o: {
                        docs: {},
                        df: 0,
                        u: {
                          docs: {},
                          df: 0,
                          n: {
                            docs: {},
                            df: 0,
                            d: {
                              docs: {},
                              df: 0,
                              s: {
                                docs: {
                                  "scouter.AlertType.OutOfBounds": { tf: 1 },
                                  "scouter.AlertZone.OutOfBounds": { tf: 1 },
                                },
                                df: 2,
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          q: {
            docs: {},
            df: 0,
            u: {
              docs: {},
              df: 0,
              a: {
                docs: {},
                df: 0,
                n: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {},
                    df: 0,
                    i: {
                      docs: {},
                      df: 0,
                      l: {
                        docs: {},
                        df: 0,
                        e: {
                          docs: {},
                          df: 0,
                          s: {
                            docs: {
                              "scouter.FeatureProfile.quantiles": { tf: 1 },
                            },
                            df: 1,
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          h: {
            docs: {},
            df: 0,
            i: {
              docs: {},
              df: 0,
              s: {
                docs: {},
                df: 0,
                t: {
                  docs: {},
                  df: 0,
                  o: {
                    docs: {},
                    df: 0,
                    g: {
                      docs: {},
                      df: 0,
                      r: {
                        docs: {},
                        df: 0,
                        a: {
                          docs: {},
                          df: 0,
                          m: {
                            docs: {
                              "scouter.FeatureProfile.histogram": { tf: 1 },
                            },
                            df: 1,
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            o: {
              docs: {},
              df: 0,
              u: {
                docs: {},
                df: 0,
                r: {
                  docs: { "scouter.CommonCron.every_hour": { tf: 1 } },
                  df: 1,
                  s: {
                    docs: {
                      "scouter.CommonCron.every_6_hours": { tf: 1 },
                      "scouter.CommonCron.every_12_hours": { tf: 1 },
                    },
                    df: 2,
                  },
                },
              },
            },
          },
          k: {
            docs: {},
            df: 0,
            i: {
              docs: {},
              df: 0,
              n: {
                docs: {},
                df: 0,
                d: { docs: { "scouter.Alert.kind": { tf: 1 } }, df: 1 },
              },
            },
          },
          z: {
            docs: {},
            df: 0,
            o: {
              docs: {},
              df: 0,
              n: {
                docs: {},
                df: 0,
                e: {
                  1: { docs: { "scouter.AlertZone.Zone1": { tf: 1 } }, df: 1 },
                  2: { docs: { "scouter.AlertZone.Zone2": { tf: 1 } }, df: 1 },
                  3: { docs: { "scouter.AlertZone.Zone3": { tf: 1 } }, df: 1 },
                  docs: { "scouter.Alert.zone": { tf: 1 } },
                  df: 1,
                },
              },
            },
          },
          n: {
            docs: {},
            df: 0,
            o: {
              docs: {},
              df: 0,
              t: {
                docs: {},
                df: 0,
                a: {
                  docs: {},
                  df: 0,
                  p: {
                    docs: {},
                    df: 0,
                    p: {
                      docs: {},
                      df: 0,
                      l: {
                        docs: {},
                        df: 0,
                        i: {
                          docs: {},
                          df: 0,
                          c: {
                            docs: {},
                            df: 0,
                            a: {
                              docs: {},
                              df: 0,
                              b: {
                                docs: {},
                                df: 0,
                                l: {
                                  docs: {},
                                  df: 0,
                                  e: {
                                    docs: {
                                      "scouter.AlertZone.NotApplicable": {
                                        tf: 1,
                                      },
                                    },
                                    df: 1,
                                  },
                                },
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              x: {
                docs: {},
                df: 0,
                t: {
                  docs: {
                    "scouter.Every30Minutes.get_next": { tf: 1 },
                    "scouter.EveryHour.get_next": { tf: 1 },
                    "scouter.Every6Hours.get_next": { tf: 1 },
                    "scouter.Every12Hours.get_next": { tf: 1 },
                    "scouter.EveryDay.get_next": { tf: 1 },
                    "scouter.EveryWeek.get_next": { tf: 1 },
                  },
                  df: 6,
                },
              },
            },
            a: {
              docs: {},
              df: 0,
              m: {
                docs: {},
                df: 0,
                e: {
                  docs: {
                    "scouter.DriftConfig.name": { tf: 1 },
                    "scouter.DriftMap.name": { tf: 1 },
                  },
                  df: 2,
                },
              },
            },
            u: {
              docs: {},
              df: 0,
              m: {
                docs: {},
                df: 0,
                p: {
                  docs: {},
                  df: 0,
                  y: {
                    docs: { "scouter.DriftMap.to_numpy": { tf: 1 } },
                    df: 1,
                  },
                },
              },
            },
          },
          r: {
            docs: {},
            df: 0,
            u: {
              docs: {},
              df: 0,
              l: {
                docs: {},
                df: 0,
                e: {
                  docs: {
                    "scouter.ProcessAlertRule.rule": { tf: 1 },
                    "scouter.PercentageAlertRule.rule": { tf: 1 },
                    "scouter.DriftConfig.alert_rule": { tf: 1 },
                  },
                  df: 3,
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              p: {
                docs: {},
                df: 0,
                o: {
                  docs: {},
                  df: 0,
                  s: {
                    docs: {},
                    df: 0,
                    i: {
                      docs: {},
                      df: 0,
                      t: {
                        docs: {},
                        df: 0,
                        o: {
                          docs: {},
                          df: 0,
                          r: {
                            docs: {},
                            df: 0,
                            y: {
                              docs: {
                                "scouter.DriftConfig.repository": { tf: 1 },
                                "scouter.DriftMap.repository": { tf: 1 },
                              },
                              df: 2,
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
              c: {
                docs: {},
                df: 0,
                o: {
                  docs: {},
                  df: 0,
                  r: {
                    docs: {},
                    df: 0,
                    d: {
                      docs: { "scouter.DriftMap.to_server_record": { tf: 1 } },
                      df: 1,
                    },
                  },
                },
              },
            },
          },
          e: {
            docs: {},
            df: 0,
            v: {
              docs: {},
              df: 0,
              e: {
                docs: {},
                df: 0,
                r: {
                  docs: {},
                  df: 0,
                  y: {
                    1: {
                      2: {
                        docs: {},
                        df: 0,
                        h: {
                          docs: {},
                          df: 0,
                          o: {
                            docs: {},
                            df: 0,
                            u: {
                              docs: {},
                              df: 0,
                              r: {
                                docs: {},
                                df: 0,
                                s: {
                                  docs: {
                                    "scouter.Every12Hours": { tf: 1 },
                                    "scouter.Every12Hours.get_next": { tf: 1 },
                                    "scouter.Every12Hours.cron": { tf: 1 },
                                  },
                                  df: 3,
                                },
                              },
                            },
                          },
                        },
                      },
                      docs: {},
                      df: 0,
                    },
                    3: {
                      0: {
                        docs: {},
                        df: 0,
                        m: {
                          docs: {},
                          df: 0,
                          i: {
                            docs: {},
                            df: 0,
                            n: {
                              docs: {},
                              df: 0,
                              u: {
                                docs: {},
                                df: 0,
                                t: {
                                  docs: {},
                                  df: 0,
                                  e: {
                                    docs: {},
                                    df: 0,
                                    s: {
                                      docs: {
                                        "scouter.Every30Minutes": { tf: 1 },
                                        "scouter.Every30Minutes.get_next": {
                                          tf: 1,
                                        },
                                        "scouter.Every30Minutes.cron": {
                                          tf: 1,
                                        },
                                      },
                                      df: 3,
                                    },
                                  },
                                },
                              },
                            },
                          },
                        },
                      },
                      docs: {},
                      df: 0,
                    },
                    6: {
                      docs: {},
                      df: 0,
                      h: {
                        docs: {},
                        df: 0,
                        o: {
                          docs: {},
                          df: 0,
                          u: {
                            docs: {},
                            df: 0,
                            r: {
                              docs: {},
                              df: 0,
                              s: {
                                docs: {
                                  "scouter.Every6Hours": { tf: 1 },
                                  "scouter.Every6Hours.get_next": { tf: 1 },
                                  "scouter.Every6Hours.cron": { tf: 1 },
                                },
                                df: 3,
                              },
                            },
                          },
                        },
                      },
                    },
                    docs: {
                      "scouter.CommonCron.every_day": { tf: 1 },
                      "scouter.CommonCron.every_week": { tf: 1 },
                      "scouter.CommonCron.every_hour": { tf: 1 },
                      "scouter.CommonCron.every_6_hours": { tf: 1 },
                      "scouter.CommonCron.every_30_minutes": { tf: 1 },
                      "scouter.CommonCron.every_12_hours": { tf: 1 },
                    },
                    df: 6,
                    h: {
                      docs: {},
                      df: 0,
                      o: {
                        docs: {},
                        df: 0,
                        u: {
                          docs: {},
                          df: 0,
                          r: {
                            docs: {
                              "scouter.EveryHour": { tf: 1 },
                              "scouter.EveryHour.get_next": { tf: 1 },
                              "scouter.EveryHour.cron": { tf: 1 },
                            },
                            df: 3,
                          },
                        },
                      },
                    },
                    d: {
                      docs: {},
                      df: 0,
                      a: {
                        docs: {},
                        df: 0,
                        y: {
                          docs: {
                            "scouter.EveryDay": { tf: 1 },
                            "scouter.EveryDay.get_next": { tf: 1 },
                            "scouter.EveryDay.cron": { tf: 1 },
                          },
                          df: 3,
                        },
                      },
                    },
                    w: {
                      docs: {},
                      df: 0,
                      e: {
                        docs: {},
                        df: 0,
                        e: {
                          docs: {},
                          df: 0,
                          k: {
                            docs: {
                              "scouter.EveryWeek": { tf: 1 },
                              "scouter.EveryWeek.get_next": { tf: 1 },
                              "scouter.EveryWeek.cron": { tf: 1 },
                            },
                            df: 3,
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          w: {
            docs: {},
            df: 0,
            e: {
              docs: {},
              df: 0,
              e: {
                docs: {},
                df: 0,
                k: {
                  docs: { "scouter.CommonCron.every_week": { tf: 1 } },
                  df: 1,
                },
              },
            },
          },
          v: {
            docs: {},
            df: 0,
            e: {
              docs: {},
              df: 0,
              r: {
                docs: {},
                df: 0,
                s: {
                  docs: {},
                  df: 0,
                  i: {
                    docs: {},
                    df: 0,
                    o: {
                      docs: {},
                      df: 0,
                      n: {
                        docs: {
                          "scouter.DriftConfig.version": { tf: 1 },
                          "scouter.DriftMap.version": { tf: 1 },
                        },
                        df: 2,
                      },
                    },
                  },
                },
              },
            },
          },
        },
      },
      fullname: {
        root: {
          1: {
            2: {
              docs: { "scouter.CommonCron.every_12_hours": { tf: 1 } },
              df: 1,
            },
            docs: {},
            df: 0,
          },
          3: {
            0: {
              docs: { "scouter.CommonCron.every_30_minutes": { tf: 1 } },
              df: 1,
            },
            docs: {},
            df: 0,
          },
          6: { docs: { "scouter.CommonCron.every_6_hours": { tf: 1 } }, df: 1 },
          docs: {
            "scouter.Profiler.__init__": { tf: 1 },
            "scouter.Drifter.__init__": { tf: 1 },
          },
          df: 2,
          s: {
            docs: {},
            df: 0,
            c: {
              docs: {},
              df: 0,
              o: {
                docs: {},
                df: 0,
                u: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {},
                    df: 0,
                    e: {
                      docs: {},
                      df: 0,
                      r: {
                        docs: {
                          scouter: { tf: 1 },
                          "scouter.Profiler": { tf: 1 },
                          "scouter.Profiler.__init__": { tf: 1 },
                          "scouter.Profiler.create_data_profile": { tf: 1 },
                          "scouter.Drifter": { tf: 1 },
                          "scouter.Drifter.__init__": { tf: 1 },
                          "scouter.Drifter.create_drift_profile": { tf: 1 },
                          "scouter.Drifter.compute_drift": { tf: 1 },
                          "scouter.Drifter.generate_alerts": { tf: 1 },
                          "scouter.DataProfile": { tf: 1 },
                          "scouter.DataProfile.model_dump_json": { tf: 1 },
                          "scouter.DataProfile.load_from_json": { tf: 1 },
                          "scouter.DataProfile.save_to_json": { tf: 1 },
                          "scouter.DataProfile.features": { tf: 1 },
                          "scouter.DriftProfile": { tf: 1 },
                          "scouter.DriftProfile.model_dump_json": { tf: 1 },
                          "scouter.DriftProfile.load_from_json": { tf: 1 },
                          "scouter.DriftProfile.save_to_json": { tf: 1 },
                          "scouter.DriftProfile.features": { tf: 1 },
                          "scouter.DriftProfile.config": { tf: 1 },
                          "scouter.FeatureDriftProfile": { tf: 1 },
                          "scouter.FeatureDriftProfile.two_lcl": { tf: 1 },
                          "scouter.FeatureDriftProfile.two_ucl": { tf: 1 },
                          "scouter.FeatureDriftProfile.one_ucl": { tf: 1 },
                          "scouter.FeatureDriftProfile.three_ucl": { tf: 1 },
                          "scouter.FeatureDriftProfile.center": { tf: 1 },
                          "scouter.FeatureDriftProfile.id": { tf: 1 },
                          "scouter.FeatureDriftProfile.one_lcl": { tf: 1 },
                          "scouter.FeatureDriftProfile.three_lcl": { tf: 1 },
                          "scouter.FeatureDriftProfile.timestamp": { tf: 1 },
                          "scouter.FeatureProfile": { tf: 1 },
                          "scouter.FeatureProfile.quantiles": { tf: 1 },
                          "scouter.FeatureProfile.distinct": { tf: 1 },
                          "scouter.FeatureProfile.stddev": { tf: 1 },
                          "scouter.FeatureProfile.timestamp": { tf: 1 },
                          "scouter.FeatureProfile.histogram": { tf: 1 },
                          "scouter.FeatureProfile.id": { tf: 1 },
                          "scouter.FeatureProfile.mean": { tf: 1 },
                          "scouter.FeatureProfile.max": { tf: 1 },
                          "scouter.FeatureProfile.min": { tf: 1 },
                          "scouter.Alert": { tf: 1 },
                          "scouter.Alert.kind": { tf: 1 },
                          "scouter.Alert.zone": { tf: 1 },
                          "scouter.AlertType": { tf: 1 },
                          "scouter.AlertType.OutOfBounds": { tf: 1 },
                          "scouter.AlertType.Consecutive": { tf: 1 },
                          "scouter.AlertType.Alternating": { tf: 1 },
                          "scouter.AlertType.AllGood": { tf: 1 },
                          "scouter.AlertType.Trend": { tf: 1 },
                          "scouter.AlertRule": { tf: 1 },
                          "scouter.AlertRule.to_str": { tf: 1 },
                          "scouter.AlertRule.process": { tf: 1 },
                          "scouter.AlertRule.percentage": { tf: 1 },
                          "scouter.AlertZone": { tf: 1 },
                          "scouter.AlertZone.Zone1": { tf: 1 },
                          "scouter.AlertZone.Zone2": { tf: 1 },
                          "scouter.AlertZone.Zone3": { tf: 1 },
                          "scouter.AlertZone.OutOfBounds": { tf: 1 },
                          "scouter.AlertZone.NotApplicable": { tf: 1 },
                          "scouter.FeatureAlerts": { tf: 1 },
                          "scouter.FeatureAlerts.model_dump_json": { tf: 1 },
                          "scouter.FeatureAlerts.features": { tf: 1 },
                          "scouter.ProcessAlertRule": { tf: 1 },
                          "scouter.ProcessAlertRule.rule": { tf: 1 },
                          "scouter.PercentageAlertRule": { tf: 1 },
                          "scouter.PercentageAlertRule.rule": { tf: 1 },
                          "scouter.CommonCrons": { tf: 1 },
                          "scouter.CommonCron": { tf: 1 },
                          "scouter.CommonCron.every_day": { tf: 1 },
                          "scouter.CommonCron.every_week": { tf: 1 },
                          "scouter.CommonCron.every_hour": { tf: 1 },
                          "scouter.CommonCron.every_6_hours": { tf: 1 },
                          "scouter.CommonCron.every_30_minutes": { tf: 1 },
                          "scouter.CommonCron.every_12_hours": { tf: 1 },
                          "scouter.Every30Minutes": { tf: 1 },
                          "scouter.Every30Minutes.get_next": { tf: 1 },
                          "scouter.Every30Minutes.cron": { tf: 1 },
                          "scouter.EveryHour": { tf: 1 },
                          "scouter.EveryHour.get_next": { tf: 1 },
                          "scouter.EveryHour.cron": { tf: 1 },
                          "scouter.Every6Hours": { tf: 1 },
                          "scouter.Every6Hours.get_next": { tf: 1 },
                          "scouter.Every6Hours.cron": { tf: 1 },
                          "scouter.Every12Hours": { tf: 1 },
                          "scouter.Every12Hours.get_next": { tf: 1 },
                          "scouter.Every12Hours.cron": { tf: 1 },
                          "scouter.EveryDay": { tf: 1 },
                          "scouter.EveryDay.get_next": { tf: 1 },
                          "scouter.EveryDay.cron": { tf: 1 },
                          "scouter.EveryWeek": { tf: 1 },
                          "scouter.EveryWeek.get_next": { tf: 1 },
                          "scouter.EveryWeek.cron": { tf: 1 },
                          "scouter.DriftConfig": { tf: 1 },
                          "scouter.DriftConfig.sample_size": { tf: 1 },
                          "scouter.DriftConfig.name": { tf: 1 },
                          "scouter.DriftConfig.sample": { tf: 1 },
                          "scouter.DriftConfig.version": { tf: 1 },
                          "scouter.DriftConfig.schedule": { tf: 1 },
                          "scouter.DriftConfig.repository": { tf: 1 },
                          "scouter.DriftConfig.alert_rule": { tf: 1 },
                          "scouter.DriftMap": { tf: 1 },
                          "scouter.DriftMap.add_feature": { tf: 1 },
                          "scouter.DriftMap.model_dump_json": { tf: 1 },
                          "scouter.DriftMap.load_from_json": { tf: 1 },
                          "scouter.DriftMap.save_to_json": { tf: 1 },
                          "scouter.DriftMap.to_server_record": { tf: 1 },
                          "scouter.DriftMap.to_numpy": { tf: 1 },
                          "scouter.DriftMap.name": { tf: 1 },
                          "scouter.DriftMap.repository": { tf: 1 },
                          "scouter.DriftMap.version": { tf: 1 },
                          "scouter.DriftMap.features": { tf: 1 },
                        },
                        df: 111,
                      },
                    },
                  },
                },
              },
              h: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  d: {
                    docs: {},
                    df: 0,
                    u: {
                      docs: {},
                      df: 0,
                      l: {
                        docs: {},
                        df: 0,
                        e: {
                          docs: { "scouter.DriftConfig.schedule": { tf: 1 } },
                          df: 1,
                        },
                      },
                    },
                  },
                },
              },
            },
            a: {
              docs: {},
              df: 0,
              v: {
                docs: {},
                df: 0,
                e: {
                  docs: {
                    "scouter.DataProfile.save_to_json": { tf: 1 },
                    "scouter.DriftProfile.save_to_json": { tf: 1 },
                    "scouter.DriftMap.save_to_json": { tf: 1 },
                  },
                  df: 3,
                },
              },
              m: {
                docs: {},
                df: 0,
                p: {
                  docs: {},
                  df: 0,
                  l: {
                    docs: {},
                    df: 0,
                    e: {
                      docs: {
                        "scouter.DriftConfig.sample_size": { tf: 1 },
                        "scouter.DriftConfig.sample": { tf: 1 },
                      },
                      df: 2,
                    },
                  },
                },
              },
            },
            t: {
              docs: {},
              df: 0,
              d: {
                docs: {},
                df: 0,
                d: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {},
                    df: 0,
                    v: {
                      docs: { "scouter.FeatureProfile.stddev": { tf: 1 } },
                      df: 1,
                    },
                  },
                },
              },
              r: { docs: { "scouter.AlertRule.to_str": { tf: 1 } }, df: 1 },
            },
            i: {
              docs: {},
              df: 0,
              z: {
                docs: {},
                df: 0,
                e: {
                  docs: { "scouter.DriftConfig.sample_size": { tf: 1 } },
                  df: 1,
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              r: {
                docs: {},
                df: 0,
                v: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {},
                    df: 0,
                    r: {
                      docs: { "scouter.DriftMap.to_server_record": { tf: 1 } },
                      df: 1,
                    },
                  },
                },
              },
            },
          },
          p: {
            docs: {},
            df: 0,
            r: {
              docs: {},
              df: 0,
              o: {
                docs: {},
                df: 0,
                f: {
                  docs: {},
                  df: 0,
                  i: {
                    docs: {},
                    df: 0,
                    l: {
                      docs: {},
                      df: 0,
                      e: {
                        docs: {
                          "scouter.Profiler.create_data_profile": { tf: 1 },
                          "scouter.Drifter.create_drift_profile": { tf: 1 },
                        },
                        df: 2,
                        r: {
                          docs: {
                            "scouter.Profiler": { tf: 1 },
                            "scouter.Profiler.__init__": { tf: 1 },
                            "scouter.Profiler.create_data_profile": { tf: 1 },
                          },
                          df: 3,
                        },
                      },
                    },
                  },
                },
                c: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {},
                    df: 0,
                    s: {
                      docs: {},
                      df: 0,
                      s: {
                        docs: { "scouter.AlertRule.process": { tf: 1 } },
                        df: 1,
                        a: {
                          docs: {},
                          df: 0,
                          l: {
                            docs: {},
                            df: 0,
                            e: {
                              docs: {},
                              df: 0,
                              r: {
                                docs: {},
                                df: 0,
                                t: {
                                  docs: {},
                                  df: 0,
                                  r: {
                                    docs: {},
                                    df: 0,
                                    u: {
                                      docs: {},
                                      df: 0,
                                      l: {
                                        docs: {},
                                        df: 0,
                                        e: {
                                          docs: {
                                            "scouter.ProcessAlertRule": {
                                              tf: 1,
                                            },
                                            "scouter.ProcessAlertRule.rule": {
                                              tf: 1,
                                            },
                                          },
                                          df: 2,
                                        },
                                      },
                                    },
                                  },
                                },
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              r: {
                docs: {},
                df: 0,
                c: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {},
                    df: 0,
                    n: {
                      docs: {},
                      df: 0,
                      t: {
                        docs: {},
                        df: 0,
                        a: {
                          docs: {},
                          df: 0,
                          g: {
                            docs: {},
                            df: 0,
                            e: {
                              docs: {
                                "scouter.AlertRule.percentage": { tf: 1 },
                              },
                              df: 1,
                              a: {
                                docs: {},
                                df: 0,
                                l: {
                                  docs: {},
                                  df: 0,
                                  e: {
                                    docs: {},
                                    df: 0,
                                    r: {
                                      docs: {},
                                      df: 0,
                                      t: {
                                        docs: {},
                                        df: 0,
                                        r: {
                                          docs: {},
                                          df: 0,
                                          u: {
                                            docs: {},
                                            df: 0,
                                            l: {
                                              docs: {},
                                              df: 0,
                                              e: {
                                                docs: {
                                                  "scouter.PercentageAlertRule":
                                                    { tf: 1 },
                                                  "scouter.PercentageAlertRule.rule":
                                                    { tf: 1 },
                                                },
                                                df: 2,
                                              },
                                            },
                                          },
                                        },
                                      },
                                    },
                                  },
                                },
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          i: {
            docs: {},
            df: 0,
            n: {
              docs: {},
              df: 0,
              i: {
                docs: {},
                df: 0,
                t: {
                  docs: {
                    "scouter.Profiler.__init__": { tf: 1 },
                    "scouter.Drifter.__init__": { tf: 1 },
                  },
                  df: 2,
                },
              },
            },
            d: {
              docs: {
                "scouter.FeatureDriftProfile.id": { tf: 1 },
                "scouter.FeatureProfile.id": { tf: 1 },
              },
              df: 2,
            },
          },
          c: {
            docs: {},
            df: 0,
            r: {
              docs: {},
              df: 0,
              e: {
                docs: {},
                df: 0,
                a: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {},
                    df: 0,
                    e: {
                      docs: {
                        "scouter.Profiler.create_data_profile": { tf: 1 },
                        "scouter.Drifter.create_drift_profile": { tf: 1 },
                      },
                      df: 2,
                    },
                  },
                },
              },
              o: {
                docs: {},
                df: 0,
                n: {
                  docs: {
                    "scouter.Every30Minutes.cron": { tf: 1 },
                    "scouter.EveryHour.cron": { tf: 1 },
                    "scouter.Every6Hours.cron": { tf: 1 },
                    "scouter.Every12Hours.cron": { tf: 1 },
                    "scouter.EveryDay.cron": { tf: 1 },
                    "scouter.EveryWeek.cron": { tf: 1 },
                  },
                  df: 6,
                },
              },
            },
            o: {
              docs: {},
              df: 0,
              m: {
                docs: {},
                df: 0,
                p: {
                  docs: {},
                  df: 0,
                  u: {
                    docs: {},
                    df: 0,
                    t: {
                      docs: {},
                      df: 0,
                      e: {
                        docs: { "scouter.Drifter.compute_drift": { tf: 1 } },
                        df: 1,
                      },
                    },
                  },
                },
                m: {
                  docs: {},
                  df: 0,
                  o: {
                    docs: {},
                    df: 0,
                    n: {
                      docs: {},
                      df: 0,
                      c: {
                        docs: {},
                        df: 0,
                        r: {
                          docs: {},
                          df: 0,
                          o: {
                            docs: {},
                            df: 0,
                            n: {
                              docs: {
                                "scouter.CommonCron": { tf: 1 },
                                "scouter.CommonCron.every_day": { tf: 1 },
                                "scouter.CommonCron.every_week": { tf: 1 },
                                "scouter.CommonCron.every_hour": { tf: 1 },
                                "scouter.CommonCron.every_6_hours": { tf: 1 },
                                "scouter.CommonCron.every_30_minutes": {
                                  tf: 1,
                                },
                                "scouter.CommonCron.every_12_hours": { tf: 1 },
                              },
                              df: 7,
                              s: {
                                docs: { "scouter.CommonCrons": { tf: 1 } },
                                df: 1,
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
              n: {
                docs: {},
                df: 0,
                f: {
                  docs: {},
                  df: 0,
                  i: {
                    docs: {},
                    df: 0,
                    g: {
                      docs: { "scouter.DriftProfile.config": { tf: 1 } },
                      df: 1,
                    },
                  },
                },
                s: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {},
                    df: 0,
                    c: {
                      docs: {},
                      df: 0,
                      u: {
                        docs: {},
                        df: 0,
                        t: {
                          docs: {},
                          df: 0,
                          i: {
                            docs: {},
                            df: 0,
                            v: {
                              docs: {},
                              df: 0,
                              e: {
                                docs: {
                                  "scouter.AlertType.Consecutive": { tf: 1 },
                                },
                                df: 1,
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              n: {
                docs: {},
                df: 0,
                t: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {},
                    df: 0,
                    r: {
                      docs: { "scouter.FeatureDriftProfile.center": { tf: 1 } },
                      df: 1,
                    },
                  },
                },
              },
            },
          },
          d: {
            docs: {},
            df: 0,
            a: {
              docs: {},
              df: 0,
              t: {
                docs: {},
                df: 0,
                a: {
                  docs: { "scouter.Profiler.create_data_profile": { tf: 1 } },
                  df: 1,
                  p: {
                    docs: {},
                    df: 0,
                    r: {
                      docs: {},
                      df: 0,
                      o: {
                        docs: {},
                        df: 0,
                        f: {
                          docs: {},
                          df: 0,
                          i: {
                            docs: {},
                            df: 0,
                            l: {
                              docs: {},
                              df: 0,
                              e: {
                                docs: {
                                  "scouter.DataProfile": { tf: 1 },
                                  "scouter.DataProfile.model_dump_json": {
                                    tf: 1,
                                  },
                                  "scouter.DataProfile.load_from_json": {
                                    tf: 1,
                                  },
                                  "scouter.DataProfile.save_to_json": { tf: 1 },
                                  "scouter.DataProfile.features": { tf: 1 },
                                },
                                df: 5,
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
              y: { docs: { "scouter.CommonCron.every_day": { tf: 1 } }, df: 1 },
            },
            r: {
              docs: {},
              df: 0,
              i: {
                docs: {},
                df: 0,
                f: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {
                      "scouter.Drifter.create_drift_profile": { tf: 1 },
                      "scouter.Drifter.compute_drift": { tf: 1 },
                    },
                    df: 2,
                    e: {
                      docs: {},
                      df: 0,
                      r: {
                        docs: {
                          "scouter.Drifter": { tf: 1 },
                          "scouter.Drifter.__init__": { tf: 1 },
                          "scouter.Drifter.create_drift_profile": { tf: 1 },
                          "scouter.Drifter.compute_drift": { tf: 1 },
                          "scouter.Drifter.generate_alerts": { tf: 1 },
                        },
                        df: 5,
                      },
                    },
                    p: {
                      docs: {},
                      df: 0,
                      r: {
                        docs: {},
                        df: 0,
                        o: {
                          docs: {},
                          df: 0,
                          f: {
                            docs: {},
                            df: 0,
                            i: {
                              docs: {},
                              df: 0,
                              l: {
                                docs: {},
                                df: 0,
                                e: {
                                  docs: {
                                    "scouter.DriftProfile": { tf: 1 },
                                    "scouter.DriftProfile.model_dump_json": {
                                      tf: 1,
                                    },
                                    "scouter.DriftProfile.load_from_json": {
                                      tf: 1,
                                    },
                                    "scouter.DriftProfile.save_to_json": {
                                      tf: 1,
                                    },
                                    "scouter.DriftProfile.features": { tf: 1 },
                                    "scouter.DriftProfile.config": { tf: 1 },
                                  },
                                  df: 6,
                                },
                              },
                            },
                          },
                        },
                      },
                    },
                    c: {
                      docs: {},
                      df: 0,
                      o: {
                        docs: {},
                        df: 0,
                        n: {
                          docs: {},
                          df: 0,
                          f: {
                            docs: {},
                            df: 0,
                            i: {
                              docs: {},
                              df: 0,
                              g: {
                                docs: {
                                  "scouter.DriftConfig": { tf: 1 },
                                  "scouter.DriftConfig.sample_size": { tf: 1 },
                                  "scouter.DriftConfig.name": { tf: 1 },
                                  "scouter.DriftConfig.sample": { tf: 1 },
                                  "scouter.DriftConfig.version": { tf: 1 },
                                  "scouter.DriftConfig.schedule": { tf: 1 },
                                  "scouter.DriftConfig.repository": { tf: 1 },
                                  "scouter.DriftConfig.alert_rule": { tf: 1 },
                                },
                                df: 8,
                              },
                            },
                          },
                        },
                      },
                    },
                    m: {
                      docs: {},
                      df: 0,
                      a: {
                        docs: {},
                        df: 0,
                        p: {
                          docs: {
                            "scouter.DriftMap": { tf: 1 },
                            "scouter.DriftMap.add_feature": { tf: 1 },
                            "scouter.DriftMap.model_dump_json": { tf: 1 },
                            "scouter.DriftMap.load_from_json": { tf: 1 },
                            "scouter.DriftMap.save_to_json": { tf: 1 },
                            "scouter.DriftMap.to_server_record": { tf: 1 },
                            "scouter.DriftMap.to_numpy": { tf: 1 },
                            "scouter.DriftMap.name": { tf: 1 },
                            "scouter.DriftMap.repository": { tf: 1 },
                            "scouter.DriftMap.version": { tf: 1 },
                            "scouter.DriftMap.features": { tf: 1 },
                          },
                          df: 11,
                        },
                      },
                    },
                  },
                },
              },
            },
            u: {
              docs: {},
              df: 0,
              m: {
                docs: {},
                df: 0,
                p: {
                  docs: {
                    "scouter.DataProfile.model_dump_json": { tf: 1 },
                    "scouter.DriftProfile.model_dump_json": { tf: 1 },
                    "scouter.FeatureAlerts.model_dump_json": { tf: 1 },
                    "scouter.DriftMap.model_dump_json": { tf: 1 },
                  },
                  df: 4,
                },
              },
            },
            i: {
              docs: {},
              df: 0,
              s: {
                docs: {},
                df: 0,
                t: {
                  docs: {},
                  df: 0,
                  i: {
                    docs: {},
                    df: 0,
                    n: {
                      docs: {},
                      df: 0,
                      c: {
                        docs: {},
                        df: 0,
                        t: {
                          docs: {
                            "scouter.FeatureProfile.distinct": { tf: 1 },
                          },
                          df: 1,
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          g: {
            docs: {},
            df: 0,
            e: {
              docs: {},
              df: 0,
              n: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  r: {
                    docs: {},
                    df: 0,
                    a: {
                      docs: {},
                      df: 0,
                      t: {
                        docs: {},
                        df: 0,
                        e: {
                          docs: {
                            "scouter.Drifter.generate_alerts": { tf: 1 },
                          },
                          df: 1,
                        },
                      },
                    },
                  },
                },
              },
              t: {
                docs: {
                  "scouter.Every30Minutes.get_next": { tf: 1 },
                  "scouter.EveryHour.get_next": { tf: 1 },
                  "scouter.Every6Hours.get_next": { tf: 1 },
                  "scouter.Every12Hours.get_next": { tf: 1 },
                  "scouter.EveryDay.get_next": { tf: 1 },
                  "scouter.EveryWeek.get_next": { tf: 1 },
                },
                df: 6,
              },
            },
          },
          a: {
            docs: {},
            df: 0,
            l: {
              docs: {},
              df: 0,
              e: {
                docs: {},
                df: 0,
                r: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {
                      "scouter.Alert": { tf: 1 },
                      "scouter.Alert.kind": { tf: 1 },
                      "scouter.Alert.zone": { tf: 1 },
                      "scouter.DriftConfig.alert_rule": { tf: 1 },
                    },
                    df: 4,
                    s: {
                      docs: { "scouter.Drifter.generate_alerts": { tf: 1 } },
                      df: 1,
                    },
                    t: {
                      docs: {},
                      df: 0,
                      y: {
                        docs: {},
                        df: 0,
                        p: {
                          docs: {},
                          df: 0,
                          e: {
                            docs: {
                              "scouter.AlertType": { tf: 1 },
                              "scouter.AlertType.OutOfBounds": { tf: 1 },
                              "scouter.AlertType.Consecutive": { tf: 1 },
                              "scouter.AlertType.Alternating": { tf: 1 },
                              "scouter.AlertType.AllGood": { tf: 1 },
                              "scouter.AlertType.Trend": { tf: 1 },
                            },
                            df: 6,
                          },
                        },
                      },
                    },
                    r: {
                      docs: {},
                      df: 0,
                      u: {
                        docs: {},
                        df: 0,
                        l: {
                          docs: {},
                          df: 0,
                          e: {
                            docs: {
                              "scouter.AlertRule": { tf: 1 },
                              "scouter.AlertRule.to_str": { tf: 1 },
                              "scouter.AlertRule.process": { tf: 1 },
                              "scouter.AlertRule.percentage": { tf: 1 },
                            },
                            df: 4,
                          },
                        },
                      },
                    },
                    z: {
                      docs: {},
                      df: 0,
                      o: {
                        docs: {},
                        df: 0,
                        n: {
                          docs: {},
                          df: 0,
                          e: {
                            docs: {
                              "scouter.AlertZone": { tf: 1 },
                              "scouter.AlertZone.Zone1": { tf: 1 },
                              "scouter.AlertZone.Zone2": { tf: 1 },
                              "scouter.AlertZone.Zone3": { tf: 1 },
                              "scouter.AlertZone.OutOfBounds": { tf: 1 },
                              "scouter.AlertZone.NotApplicable": { tf: 1 },
                            },
                            df: 6,
                          },
                        },
                      },
                    },
                  },
                },
              },
              t: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  r: {
                    docs: {},
                    df: 0,
                    n: {
                      docs: {},
                      df: 0,
                      a: {
                        docs: {},
                        df: 0,
                        t: {
                          docs: {},
                          df: 0,
                          i: {
                            docs: {},
                            df: 0,
                            n: {
                              docs: {},
                              df: 0,
                              g: {
                                docs: {
                                  "scouter.AlertType.Alternating": { tf: 1 },
                                },
                                df: 1,
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
              l: {
                docs: {},
                df: 0,
                g: {
                  docs: {},
                  df: 0,
                  o: {
                    docs: {},
                    df: 0,
                    o: {
                      docs: {},
                      df: 0,
                      d: {
                        docs: { "scouter.AlertType.AllGood": { tf: 1 } },
                        df: 1,
                      },
                    },
                  },
                },
              },
            },
            d: {
              docs: {},
              df: 0,
              d: { docs: { "scouter.DriftMap.add_feature": { tf: 1 } }, df: 1 },
            },
          },
          m: {
            docs: {},
            df: 0,
            o: {
              docs: {},
              df: 0,
              d: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  l: {
                    docs: {
                      "scouter.DataProfile.model_dump_json": { tf: 1 },
                      "scouter.DriftProfile.model_dump_json": { tf: 1 },
                      "scouter.FeatureAlerts.model_dump_json": { tf: 1 },
                      "scouter.DriftMap.model_dump_json": { tf: 1 },
                    },
                    df: 4,
                  },
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              a: {
                docs: {},
                df: 0,
                n: {
                  docs: { "scouter.FeatureProfile.mean": { tf: 1 } },
                  df: 1,
                },
              },
            },
            a: {
              docs: {},
              df: 0,
              x: { docs: { "scouter.FeatureProfile.max": { tf: 1 } }, df: 1 },
            },
            i: {
              docs: {},
              df: 0,
              n: {
                docs: { "scouter.FeatureProfile.min": { tf: 1 } },
                df: 1,
                u: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {},
                    df: 0,
                    e: {
                      docs: {},
                      df: 0,
                      s: {
                        docs: {
                          "scouter.CommonCron.every_30_minutes": { tf: 1 },
                        },
                        df: 1,
                      },
                    },
                  },
                },
              },
            },
          },
          j: {
            docs: {},
            df: 0,
            s: {
              docs: {},
              df: 0,
              o: {
                docs: {},
                df: 0,
                n: {
                  docs: {
                    "scouter.DataProfile.model_dump_json": { tf: 1 },
                    "scouter.DataProfile.load_from_json": { tf: 1 },
                    "scouter.DataProfile.save_to_json": { tf: 1 },
                    "scouter.DriftProfile.model_dump_json": { tf: 1 },
                    "scouter.DriftProfile.load_from_json": { tf: 1 },
                    "scouter.DriftProfile.save_to_json": { tf: 1 },
                    "scouter.FeatureAlerts.model_dump_json": { tf: 1 },
                    "scouter.DriftMap.model_dump_json": { tf: 1 },
                    "scouter.DriftMap.load_from_json": { tf: 1 },
                    "scouter.DriftMap.save_to_json": { tf: 1 },
                  },
                  df: 10,
                },
              },
            },
          },
          l: {
            docs: {},
            df: 0,
            o: {
              docs: {},
              df: 0,
              a: {
                docs: {},
                df: 0,
                d: {
                  docs: {
                    "scouter.DataProfile.load_from_json": { tf: 1 },
                    "scouter.DriftProfile.load_from_json": { tf: 1 },
                    "scouter.DriftMap.load_from_json": { tf: 1 },
                  },
                  df: 3,
                },
              },
            },
            c: {
              docs: {},
              df: 0,
              l: {
                docs: {
                  "scouter.FeatureDriftProfile.two_lcl": { tf: 1 },
                  "scouter.FeatureDriftProfile.one_lcl": { tf: 1 },
                  "scouter.FeatureDriftProfile.three_lcl": { tf: 1 },
                },
                df: 3,
              },
            },
          },
          f: {
            docs: {},
            df: 0,
            r: {
              docs: {},
              df: 0,
              o: {
                docs: {},
                df: 0,
                m: {
                  docs: {
                    "scouter.DataProfile.load_from_json": { tf: 1 },
                    "scouter.DriftProfile.load_from_json": { tf: 1 },
                    "scouter.DriftMap.load_from_json": { tf: 1 },
                  },
                  df: 3,
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              a: {
                docs: {},
                df: 0,
                t: {
                  docs: {},
                  df: 0,
                  u: {
                    docs: {},
                    df: 0,
                    r: {
                      docs: {},
                      df: 0,
                      e: {
                        docs: { "scouter.DriftMap.add_feature": { tf: 1 } },
                        df: 1,
                        s: {
                          docs: {
                            "scouter.DataProfile.features": { tf: 1 },
                            "scouter.DriftProfile.features": { tf: 1 },
                            "scouter.FeatureAlerts.features": { tf: 1 },
                            "scouter.DriftMap.features": { tf: 1 },
                          },
                          df: 4,
                        },
                        d: {
                          docs: {},
                          df: 0,
                          r: {
                            docs: {},
                            df: 0,
                            i: {
                              docs: {},
                              df: 0,
                              f: {
                                docs: {},
                                df: 0,
                                t: {
                                  docs: {},
                                  df: 0,
                                  p: {
                                    docs: {},
                                    df: 0,
                                    r: {
                                      docs: {},
                                      df: 0,
                                      o: {
                                        docs: {},
                                        df: 0,
                                        f: {
                                          docs: {},
                                          df: 0,
                                          i: {
                                            docs: {},
                                            df: 0,
                                            l: {
                                              docs: {},
                                              df: 0,
                                              e: {
                                                docs: {
                                                  "scouter.FeatureDriftProfile":
                                                    { tf: 1 },
                                                  "scouter.FeatureDriftProfile.two_lcl":
                                                    { tf: 1 },
                                                  "scouter.FeatureDriftProfile.two_ucl":
                                                    { tf: 1 },
                                                  "scouter.FeatureDriftProfile.one_ucl":
                                                    { tf: 1 },
                                                  "scouter.FeatureDriftProfile.three_ucl":
                                                    { tf: 1 },
                                                  "scouter.FeatureDriftProfile.center":
                                                    { tf: 1 },
                                                  "scouter.FeatureDriftProfile.id":
                                                    { tf: 1 },
                                                  "scouter.FeatureDriftProfile.one_lcl":
                                                    { tf: 1 },
                                                  "scouter.FeatureDriftProfile.three_lcl":
                                                    { tf: 1 },
                                                  "scouter.FeatureDriftProfile.timestamp":
                                                    { tf: 1 },
                                                },
                                                df: 10,
                                              },
                                            },
                                          },
                                        },
                                      },
                                    },
                                  },
                                },
                              },
                            },
                          },
                          a: {
                            docs: {},
                            df: 0,
                            t: {
                              docs: {},
                              df: 0,
                              a: {
                                docs: {},
                                df: 0,
                                p: {
                                  docs: {},
                                  df: 0,
                                  r: {
                                    docs: {},
                                    df: 0,
                                    o: {
                                      docs: {},
                                      df: 0,
                                      f: {
                                        docs: {},
                                        df: 0,
                                        i: {
                                          docs: {},
                                          df: 0,
                                          l: {
                                            docs: {},
                                            df: 0,
                                            e: {
                                              docs: {
                                                "scouter.FeatureProfile": {
                                                  tf: 1,
                                                },
                                                "scouter.FeatureProfile.quantiles":
                                                  { tf: 1 },
                                                "scouter.FeatureProfile.distinct":
                                                  { tf: 1 },
                                                "scouter.FeatureProfile.stddev":
                                                  { tf: 1 },
                                                "scouter.FeatureProfile.timestamp":
                                                  { tf: 1 },
                                                "scouter.FeatureProfile.histogram":
                                                  { tf: 1 },
                                                "scouter.FeatureProfile.id": {
                                                  tf: 1,
                                                },
                                                "scouter.FeatureProfile.mean": {
                                                  tf: 1,
                                                },
                                                "scouter.FeatureProfile.max": {
                                                  tf: 1,
                                                },
                                                "scouter.FeatureProfile.min": {
                                                  tf: 1,
                                                },
                                              },
                                              df: 10,
                                            },
                                          },
                                        },
                                      },
                                    },
                                  },
                                },
                              },
                            },
                          },
                        },
                        a: {
                          docs: {},
                          df: 0,
                          l: {
                            docs: {},
                            df: 0,
                            e: {
                              docs: {},
                              df: 0,
                              r: {
                                docs: {},
                                df: 0,
                                t: {
                                  docs: {},
                                  df: 0,
                                  s: {
                                    docs: {
                                      "scouter.FeatureAlerts": { tf: 1 },
                                      "scouter.FeatureAlerts.model_dump_json": {
                                        tf: 1,
                                      },
                                      "scouter.FeatureAlerts.features": {
                                        tf: 1,
                                      },
                                    },
                                    df: 3,
                                  },
                                },
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          t: {
            docs: {},
            df: 0,
            o: {
              docs: {
                "scouter.DataProfile.save_to_json": { tf: 1 },
                "scouter.DriftProfile.save_to_json": { tf: 1 },
                "scouter.AlertRule.to_str": { tf: 1 },
                "scouter.DriftMap.save_to_json": { tf: 1 },
                "scouter.DriftMap.to_server_record": { tf: 1 },
                "scouter.DriftMap.to_numpy": { tf: 1 },
              },
              df: 6,
            },
            w: {
              docs: {},
              df: 0,
              o: {
                docs: {
                  "scouter.FeatureDriftProfile.two_lcl": { tf: 1 },
                  "scouter.FeatureDriftProfile.two_ucl": { tf: 1 },
                },
                df: 2,
              },
            },
            h: {
              docs: {},
              df: 0,
              r: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {
                      "scouter.FeatureDriftProfile.three_ucl": { tf: 1 },
                      "scouter.FeatureDriftProfile.three_lcl": { tf: 1 },
                    },
                    df: 2,
                  },
                },
              },
            },
            i: {
              docs: {},
              df: 0,
              m: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  s: {
                    docs: {},
                    df: 0,
                    t: {
                      docs: {},
                      df: 0,
                      a: {
                        docs: {},
                        df: 0,
                        m: {
                          docs: {},
                          df: 0,
                          p: {
                            docs: {
                              "scouter.FeatureDriftProfile.timestamp": {
                                tf: 1,
                              },
                              "scouter.FeatureProfile.timestamp": { tf: 1 },
                            },
                            df: 2,
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            r: {
              docs: {},
              df: 0,
              e: {
                docs: {},
                df: 0,
                n: {
                  docs: {},
                  df: 0,
                  d: { docs: { "scouter.AlertType.Trend": { tf: 1 } }, df: 1 },
                },
              },
            },
          },
          u: {
            docs: {},
            df: 0,
            c: {
              docs: {},
              df: 0,
              l: {
                docs: {
                  "scouter.FeatureDriftProfile.two_ucl": { tf: 1 },
                  "scouter.FeatureDriftProfile.one_ucl": { tf: 1 },
                  "scouter.FeatureDriftProfile.three_ucl": { tf: 1 },
                },
                df: 3,
              },
            },
          },
          o: {
            docs: {},
            df: 0,
            n: {
              docs: {},
              df: 0,
              e: {
                docs: {
                  "scouter.FeatureDriftProfile.one_ucl": { tf: 1 },
                  "scouter.FeatureDriftProfile.one_lcl": { tf: 1 },
                },
                df: 2,
              },
            },
            u: {
              docs: {},
              df: 0,
              t: {
                docs: {},
                df: 0,
                o: {
                  docs: {},
                  df: 0,
                  f: {
                    docs: {},
                    df: 0,
                    b: {
                      docs: {},
                      df: 0,
                      o: {
                        docs: {},
                        df: 0,
                        u: {
                          docs: {},
                          df: 0,
                          n: {
                            docs: {},
                            df: 0,
                            d: {
                              docs: {},
                              df: 0,
                              s: {
                                docs: {
                                  "scouter.AlertType.OutOfBounds": { tf: 1 },
                                  "scouter.AlertZone.OutOfBounds": { tf: 1 },
                                },
                                df: 2,
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          q: {
            docs: {},
            df: 0,
            u: {
              docs: {},
              df: 0,
              a: {
                docs: {},
                df: 0,
                n: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {},
                    df: 0,
                    i: {
                      docs: {},
                      df: 0,
                      l: {
                        docs: {},
                        df: 0,
                        e: {
                          docs: {},
                          df: 0,
                          s: {
                            docs: {
                              "scouter.FeatureProfile.quantiles": { tf: 1 },
                            },
                            df: 1,
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          h: {
            docs: {},
            df: 0,
            i: {
              docs: {},
              df: 0,
              s: {
                docs: {},
                df: 0,
                t: {
                  docs: {},
                  df: 0,
                  o: {
                    docs: {},
                    df: 0,
                    g: {
                      docs: {},
                      df: 0,
                      r: {
                        docs: {},
                        df: 0,
                        a: {
                          docs: {},
                          df: 0,
                          m: {
                            docs: {
                              "scouter.FeatureProfile.histogram": { tf: 1 },
                            },
                            df: 1,
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            o: {
              docs: {},
              df: 0,
              u: {
                docs: {},
                df: 0,
                r: {
                  docs: { "scouter.CommonCron.every_hour": { tf: 1 } },
                  df: 1,
                  s: {
                    docs: {
                      "scouter.CommonCron.every_6_hours": { tf: 1 },
                      "scouter.CommonCron.every_12_hours": { tf: 1 },
                    },
                    df: 2,
                  },
                },
              },
            },
          },
          k: {
            docs: {},
            df: 0,
            i: {
              docs: {},
              df: 0,
              n: {
                docs: {},
                df: 0,
                d: { docs: { "scouter.Alert.kind": { tf: 1 } }, df: 1 },
              },
            },
          },
          z: {
            docs: {},
            df: 0,
            o: {
              docs: {},
              df: 0,
              n: {
                docs: {},
                df: 0,
                e: {
                  1: { docs: { "scouter.AlertZone.Zone1": { tf: 1 } }, df: 1 },
                  2: { docs: { "scouter.AlertZone.Zone2": { tf: 1 } }, df: 1 },
                  3: { docs: { "scouter.AlertZone.Zone3": { tf: 1 } }, df: 1 },
                  docs: { "scouter.Alert.zone": { tf: 1 } },
                  df: 1,
                },
              },
            },
          },
          n: {
            docs: {},
            df: 0,
            o: {
              docs: {},
              df: 0,
              t: {
                docs: {},
                df: 0,
                a: {
                  docs: {},
                  df: 0,
                  p: {
                    docs: {},
                    df: 0,
                    p: {
                      docs: {},
                      df: 0,
                      l: {
                        docs: {},
                        df: 0,
                        i: {
                          docs: {},
                          df: 0,
                          c: {
                            docs: {},
                            df: 0,
                            a: {
                              docs: {},
                              df: 0,
                              b: {
                                docs: {},
                                df: 0,
                                l: {
                                  docs: {},
                                  df: 0,
                                  e: {
                                    docs: {
                                      "scouter.AlertZone.NotApplicable": {
                                        tf: 1,
                                      },
                                    },
                                    df: 1,
                                  },
                                },
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              x: {
                docs: {},
                df: 0,
                t: {
                  docs: {
                    "scouter.Every30Minutes.get_next": { tf: 1 },
                    "scouter.EveryHour.get_next": { tf: 1 },
                    "scouter.Every6Hours.get_next": { tf: 1 },
                    "scouter.Every12Hours.get_next": { tf: 1 },
                    "scouter.EveryDay.get_next": { tf: 1 },
                    "scouter.EveryWeek.get_next": { tf: 1 },
                  },
                  df: 6,
                },
              },
            },
            a: {
              docs: {},
              df: 0,
              m: {
                docs: {},
                df: 0,
                e: {
                  docs: {
                    "scouter.DriftConfig.name": { tf: 1 },
                    "scouter.DriftMap.name": { tf: 1 },
                  },
                  df: 2,
                },
              },
            },
            u: {
              docs: {},
              df: 0,
              m: {
                docs: {},
                df: 0,
                p: {
                  docs: {},
                  df: 0,
                  y: {
                    docs: { "scouter.DriftMap.to_numpy": { tf: 1 } },
                    df: 1,
                  },
                },
              },
            },
          },
          r: {
            docs: {},
            df: 0,
            u: {
              docs: {},
              df: 0,
              l: {
                docs: {},
                df: 0,
                e: {
                  docs: {
                    "scouter.ProcessAlertRule.rule": { tf: 1 },
                    "scouter.PercentageAlertRule.rule": { tf: 1 },
                    "scouter.DriftConfig.alert_rule": { tf: 1 },
                  },
                  df: 3,
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              p: {
                docs: {},
                df: 0,
                o: {
                  docs: {},
                  df: 0,
                  s: {
                    docs: {},
                    df: 0,
                    i: {
                      docs: {},
                      df: 0,
                      t: {
                        docs: {},
                        df: 0,
                        o: {
                          docs: {},
                          df: 0,
                          r: {
                            docs: {},
                            df: 0,
                            y: {
                              docs: {
                                "scouter.DriftConfig.repository": { tf: 1 },
                                "scouter.DriftMap.repository": { tf: 1 },
                              },
                              df: 2,
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
              c: {
                docs: {},
                df: 0,
                o: {
                  docs: {},
                  df: 0,
                  r: {
                    docs: {},
                    df: 0,
                    d: {
                      docs: { "scouter.DriftMap.to_server_record": { tf: 1 } },
                      df: 1,
                    },
                  },
                },
              },
            },
          },
          e: {
            docs: {},
            df: 0,
            v: {
              docs: {},
              df: 0,
              e: {
                docs: {},
                df: 0,
                r: {
                  docs: {},
                  df: 0,
                  y: {
                    1: {
                      2: {
                        docs: {},
                        df: 0,
                        h: {
                          docs: {},
                          df: 0,
                          o: {
                            docs: {},
                            df: 0,
                            u: {
                              docs: {},
                              df: 0,
                              r: {
                                docs: {},
                                df: 0,
                                s: {
                                  docs: {
                                    "scouter.Every12Hours": { tf: 1 },
                                    "scouter.Every12Hours.get_next": { tf: 1 },
                                    "scouter.Every12Hours.cron": { tf: 1 },
                                  },
                                  df: 3,
                                },
                              },
                            },
                          },
                        },
                      },
                      docs: {},
                      df: 0,
                    },
                    3: {
                      0: {
                        docs: {},
                        df: 0,
                        m: {
                          docs: {},
                          df: 0,
                          i: {
                            docs: {},
                            df: 0,
                            n: {
                              docs: {},
                              df: 0,
                              u: {
                                docs: {},
                                df: 0,
                                t: {
                                  docs: {},
                                  df: 0,
                                  e: {
                                    docs: {},
                                    df: 0,
                                    s: {
                                      docs: {
                                        "scouter.Every30Minutes": { tf: 1 },
                                        "scouter.Every30Minutes.get_next": {
                                          tf: 1,
                                        },
                                        "scouter.Every30Minutes.cron": {
                                          tf: 1,
                                        },
                                      },
                                      df: 3,
                                    },
                                  },
                                },
                              },
                            },
                          },
                        },
                      },
                      docs: {},
                      df: 0,
                    },
                    6: {
                      docs: {},
                      df: 0,
                      h: {
                        docs: {},
                        df: 0,
                        o: {
                          docs: {},
                          df: 0,
                          u: {
                            docs: {},
                            df: 0,
                            r: {
                              docs: {},
                              df: 0,
                              s: {
                                docs: {
                                  "scouter.Every6Hours": { tf: 1 },
                                  "scouter.Every6Hours.get_next": { tf: 1 },
                                  "scouter.Every6Hours.cron": { tf: 1 },
                                },
                                df: 3,
                              },
                            },
                          },
                        },
                      },
                    },
                    docs: {
                      "scouter.CommonCron.every_day": { tf: 1 },
                      "scouter.CommonCron.every_week": { tf: 1 },
                      "scouter.CommonCron.every_hour": { tf: 1 },
                      "scouter.CommonCron.every_6_hours": { tf: 1 },
                      "scouter.CommonCron.every_30_minutes": { tf: 1 },
                      "scouter.CommonCron.every_12_hours": { tf: 1 },
                    },
                    df: 6,
                    h: {
                      docs: {},
                      df: 0,
                      o: {
                        docs: {},
                        df: 0,
                        u: {
                          docs: {},
                          df: 0,
                          r: {
                            docs: {
                              "scouter.EveryHour": { tf: 1 },
                              "scouter.EveryHour.get_next": { tf: 1 },
                              "scouter.EveryHour.cron": { tf: 1 },
                            },
                            df: 3,
                          },
                        },
                      },
                    },
                    d: {
                      docs: {},
                      df: 0,
                      a: {
                        docs: {},
                        df: 0,
                        y: {
                          docs: {
                            "scouter.EveryDay": { tf: 1 },
                            "scouter.EveryDay.get_next": { tf: 1 },
                            "scouter.EveryDay.cron": { tf: 1 },
                          },
                          df: 3,
                        },
                      },
                    },
                    w: {
                      docs: {},
                      df: 0,
                      e: {
                        docs: {},
                        df: 0,
                        e: {
                          docs: {},
                          df: 0,
                          k: {
                            docs: {
                              "scouter.EveryWeek": { tf: 1 },
                              "scouter.EveryWeek.get_next": { tf: 1 },
                              "scouter.EveryWeek.cron": { tf: 1 },
                            },
                            df: 3,
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          w: {
            docs: {},
            df: 0,
            e: {
              docs: {},
              df: 0,
              e: {
                docs: {},
                df: 0,
                k: {
                  docs: { "scouter.CommonCron.every_week": { tf: 1 } },
                  df: 1,
                },
              },
            },
          },
          v: {
            docs: {},
            df: 0,
            e: {
              docs: {},
              df: 0,
              r: {
                docs: {},
                df: 0,
                s: {
                  docs: {},
                  df: 0,
                  i: {
                    docs: {},
                    df: 0,
                    o: {
                      docs: {},
                      df: 0,
                      n: {
                        docs: {
                          "scouter.DriftConfig.version": { tf: 1 },
                          "scouter.DriftMap.version": { tf: 1 },
                        },
                        df: 2,
                      },
                    },
                  },
                },
              },
            },
          },
        },
      },
      annotation: { root: { docs: {}, df: 0 } },
      default_value: {
        root: {
          1: { docs: { "scouter.AlertZone.Zone1": { tf: 1 } }, df: 1 },
          2: { docs: { "scouter.AlertZone.Zone2": { tf: 1 } }, df: 1 },
          3: { docs: { "scouter.AlertZone.Zone3": { tf: 1 } }, df: 1 },
          docs: {
            "scouter.AlertType.OutOfBounds": { tf: 1.4142135623730951 },
            "scouter.AlertType.Consecutive": { tf: 1.4142135623730951 },
            "scouter.AlertType.Alternating": { tf: 1.4142135623730951 },
            "scouter.AlertType.AllGood": { tf: 1.4142135623730951 },
            "scouter.AlertType.Trend": { tf: 1.4142135623730951 },
            "scouter.AlertZone.Zone1": { tf: 1.4142135623730951 },
            "scouter.AlertZone.Zone2": { tf: 1.4142135623730951 },
            "scouter.AlertZone.Zone3": { tf: 1.4142135623730951 },
            "scouter.AlertZone.OutOfBounds": { tf: 1.4142135623730951 },
            "scouter.AlertZone.NotApplicable": { tf: 1.4142135623730951 },
            "scouter.CommonCrons": { tf: 1.4142135623730951 },
          },
          df: 11,
          l: {
            docs: {},
            df: 0,
            t: {
              docs: {
                "scouter.AlertType.OutOfBounds": { tf: 1 },
                "scouter.AlertType.Consecutive": { tf: 1 },
                "scouter.AlertType.Alternating": { tf: 1 },
                "scouter.AlertType.AllGood": { tf: 1 },
                "scouter.AlertType.Trend": { tf: 1 },
                "scouter.AlertZone.Zone1": { tf: 1 },
                "scouter.AlertZone.Zone2": { tf: 1 },
                "scouter.AlertZone.Zone3": { tf: 1 },
                "scouter.AlertZone.OutOfBounds": { tf: 1 },
                "scouter.AlertZone.NotApplicable": { tf: 1 },
                "scouter.CommonCrons": { tf: 1 },
              },
              df: 11,
            },
          },
          a: {
            docs: {},
            df: 0,
            l: {
              docs: {},
              df: 0,
              e: {
                docs: {},
                df: 0,
                r: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {},
                    df: 0,
                    t: {
                      docs: {},
                      df: 0,
                      y: {
                        docs: {},
                        df: 0,
                        p: {
                          docs: {},
                          df: 0,
                          e: {
                            docs: {
                              "scouter.AlertType.OutOfBounds": { tf: 1 },
                              "scouter.AlertType.Consecutive": { tf: 1 },
                              "scouter.AlertType.Alternating": { tf: 1 },
                              "scouter.AlertType.AllGood": { tf: 1 },
                              "scouter.AlertType.Trend": { tf: 1 },
                            },
                            df: 5,
                          },
                        },
                      },
                    },
                    z: {
                      docs: {},
                      df: 0,
                      o: {
                        docs: {},
                        df: 0,
                        n: {
                          docs: {},
                          df: 0,
                          e: {
                            docs: {
                              "scouter.AlertZone.Zone1": { tf: 1 },
                              "scouter.AlertZone.Zone2": { tf: 1 },
                              "scouter.AlertZone.Zone3": { tf: 1 },
                              "scouter.AlertZone.OutOfBounds": { tf: 1 },
                              "scouter.AlertZone.NotApplicable": { tf: 1 },
                            },
                            df: 5,
                          },
                        },
                      },
                    },
                  },
                },
              },
              t: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  r: {
                    docs: {},
                    df: 0,
                    n: {
                      docs: {},
                      df: 0,
                      a: {
                        docs: {},
                        df: 0,
                        t: {
                          docs: {},
                          df: 0,
                          i: {
                            docs: {},
                            df: 0,
                            n: {
                              docs: {},
                              df: 0,
                              g: {
                                docs: {
                                  "scouter.AlertType.Alternating": {
                                    tf: 1.4142135623730951,
                                  },
                                },
                                df: 1,
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
              l: {
                docs: { "scouter.AlertType.AllGood": { tf: 1 } },
                df: 1,
                g: {
                  docs: {},
                  df: 0,
                  o: {
                    docs: {},
                    df: 0,
                    o: {
                      docs: {},
                      df: 0,
                      d: {
                        docs: { "scouter.AlertType.AllGood": { tf: 1 } },
                        df: 1,
                      },
                    },
                  },
                },
              },
            },
          },
          o: {
            docs: {},
            df: 0,
            u: {
              docs: {},
              df: 0,
              t: {
                docs: {
                  "scouter.AlertType.OutOfBounds": { tf: 1 },
                  "scouter.AlertZone.OutOfBounds": { tf: 1 },
                },
                df: 2,
                o: {
                  docs: {},
                  df: 0,
                  f: {
                    docs: {},
                    df: 0,
                    b: {
                      docs: {},
                      df: 0,
                      o: {
                        docs: {},
                        df: 0,
                        u: {
                          docs: {},
                          df: 0,
                          n: {
                            docs: {},
                            df: 0,
                            d: {
                              docs: {},
                              df: 0,
                              s: {
                                docs: {
                                  "scouter.AlertType.OutOfBounds": { tf: 1 },
                                  "scouter.AlertZone.OutOfBounds": { tf: 1 },
                                },
                                df: 2,
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            f: {
              docs: {
                "scouter.AlertType.OutOfBounds": { tf: 1 },
                "scouter.AlertZone.OutOfBounds": { tf: 1 },
              },
              df: 2,
            },
            b: {
              docs: {},
              df: 0,
              j: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  c: {
                    docs: {},
                    df: 0,
                    t: { docs: { "scouter.CommonCrons": { tf: 1 } }, df: 1 },
                  },
                },
              },
            },
          },
          x: {
            2: {
              7: {
                docs: {
                  "scouter.AlertType.OutOfBounds": { tf: 1.4142135623730951 },
                  "scouter.AlertType.Consecutive": { tf: 1.4142135623730951 },
                  "scouter.AlertType.Alternating": { tf: 1.4142135623730951 },
                  "scouter.AlertType.AllGood": { tf: 1.4142135623730951 },
                  "scouter.AlertType.Trend": { tf: 1.4142135623730951 },
                  "scouter.AlertZone.Zone1": { tf: 1.4142135623730951 },
                  "scouter.AlertZone.Zone2": { tf: 1.4142135623730951 },
                  "scouter.AlertZone.Zone3": { tf: 1.4142135623730951 },
                  "scouter.AlertZone.OutOfBounds": { tf: 1.4142135623730951 },
                  "scouter.AlertZone.NotApplicable": { tf: 1.4142135623730951 },
                },
                df: 10,
              },
              docs: {},
              df: 0,
            },
            docs: {},
            df: 0,
          },
          b: {
            docs: {},
            df: 0,
            o: {
              docs: {},
              df: 0,
              u: {
                docs: {},
                df: 0,
                n: {
                  docs: {},
                  df: 0,
                  d: {
                    docs: {},
                    df: 0,
                    s: {
                      docs: {
                        "scouter.AlertType.OutOfBounds": { tf: 1 },
                        "scouter.AlertZone.OutOfBounds": { tf: 1 },
                      },
                      df: 2,
                    },
                  },
                },
              },
            },
            u: {
              docs: {},
              df: 0,
              i: {
                docs: {},
                df: 0,
                l: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {},
                    df: 0,
                    i: {
                      docs: {},
                      df: 0,
                      n: {
                        docs: {},
                        df: 0,
                        s: {
                          docs: { "scouter.CommonCrons": { tf: 1 } },
                          df: 1,
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          g: {
            docs: {},
            df: 0,
            t: {
              docs: {
                "scouter.AlertType.OutOfBounds": { tf: 1 },
                "scouter.AlertType.Consecutive": { tf: 1 },
                "scouter.AlertType.Alternating": { tf: 1 },
                "scouter.AlertType.AllGood": { tf: 1 },
                "scouter.AlertType.Trend": { tf: 1 },
                "scouter.AlertZone.Zone1": { tf: 1 },
                "scouter.AlertZone.Zone2": { tf: 1 },
                "scouter.AlertZone.Zone3": { tf: 1 },
                "scouter.AlertZone.OutOfBounds": { tf: 1 },
                "scouter.AlertZone.NotApplicable": { tf: 1 },
                "scouter.CommonCrons": { tf: 1 },
              },
              df: 11,
            },
            o: {
              docs: {},
              df: 0,
              o: {
                docs: {},
                df: 0,
                d: { docs: { "scouter.AlertType.AllGood": { tf: 1 } }, df: 1 },
              },
            },
          },
          c: {
            docs: {},
            df: 0,
            o: {
              docs: {},
              df: 0,
              n: {
                docs: {},
                df: 0,
                s: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {},
                    df: 0,
                    c: {
                      docs: {},
                      df: 0,
                      u: {
                        docs: {},
                        df: 0,
                        t: {
                          docs: {},
                          df: 0,
                          i: {
                            docs: {},
                            df: 0,
                            v: {
                              docs: {},
                              df: 0,
                              e: {
                                docs: {
                                  "scouter.AlertType.Consecutive": {
                                    tf: 1.4142135623730951,
                                  },
                                },
                                df: 1,
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
              m: {
                docs: {},
                df: 0,
                m: {
                  docs: {},
                  df: 0,
                  o: {
                    docs: {},
                    df: 0,
                    n: {
                      docs: {},
                      df: 0,
                      c: {
                        docs: {},
                        df: 0,
                        r: {
                          docs: {},
                          df: 0,
                          o: {
                            docs: {},
                            df: 0,
                            n: {
                              docs: { "scouter.CommonCrons": { tf: 1 } },
                              df: 1,
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          t: {
            docs: {},
            df: 0,
            r: {
              docs: {},
              df: 0,
              e: {
                docs: {},
                df: 0,
                n: {
                  docs: {},
                  df: 0,
                  d: {
                    docs: {
                      "scouter.AlertType.Trend": { tf: 1.4142135623730951 },
                    },
                    df: 1,
                  },
                },
              },
            },
          },
          z: {
            docs: {},
            df: 0,
            o: {
              docs: {},
              df: 0,
              n: {
                docs: {},
                df: 0,
                e: {
                  1: { docs: { "scouter.AlertZone.Zone1": { tf: 1 } }, df: 1 },
                  2: { docs: { "scouter.AlertZone.Zone2": { tf: 1 } }, df: 1 },
                  3: { docs: { "scouter.AlertZone.Zone3": { tf: 1 } }, df: 1 },
                  docs: {
                    "scouter.AlertZone.Zone1": { tf: 1 },
                    "scouter.AlertZone.Zone2": { tf: 1 },
                    "scouter.AlertZone.Zone3": { tf: 1 },
                  },
                  df: 3,
                },
              },
            },
          },
          n: {
            docs: {},
            df: 0,
            o: {
              docs: {},
              df: 0,
              t: {
                docs: {},
                df: 0,
                a: {
                  docs: {},
                  df: 0,
                  p: {
                    docs: {},
                    df: 0,
                    p: {
                      docs: {},
                      df: 0,
                      l: {
                        docs: {},
                        df: 0,
                        i: {
                          docs: {},
                          df: 0,
                          c: {
                            docs: {},
                            df: 0,
                            a: {
                              docs: {},
                              df: 0,
                              b: {
                                docs: {},
                                df: 0,
                                l: {
                                  docs: {},
                                  df: 0,
                                  e: {
                                    docs: {
                                      "scouter.AlertZone.NotApplicable": {
                                        tf: 1,
                                      },
                                    },
                                    df: 1,
                                  },
                                },
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            a: {
              docs: { "scouter.AlertZone.NotApplicable": { tf: 1 } },
              df: 1,
            },
          },
        },
      },
      signature: {
        root: {
          2: {
            0: {
              docs: { "scouter.Profiler.create_data_profile": { tf: 1 } },
              df: 1,
            },
            docs: {},
            df: 0,
          },
          docs: {
            "scouter.Profiler.__init__": { tf: 2 },
            "scouter.Profiler.create_data_profile": { tf: 11 },
            "scouter.Drifter.__init__": { tf: 2 },
            "scouter.Drifter.create_drift_profile": { tf: 10.723805294763608 },
            "scouter.Drifter.compute_drift": { tf: 10.723805294763608 },
            "scouter.Drifter.generate_alerts": { tf: 8.54400374531753 },
            "scouter.DataProfile.model_dump_json": { tf: 3.872983346207417 },
            "scouter.DataProfile.load_from_json": { tf: 3.1622776601683795 },
            "scouter.DataProfile.save_to_json": { tf: 4.795831523312719 },
            "scouter.DriftProfile.model_dump_json": { tf: 3.872983346207417 },
            "scouter.DriftProfile.load_from_json": { tf: 3.1622776601683795 },
            "scouter.DriftProfile.save_to_json": { tf: 4.795831523312719 },
            "scouter.AlertRule.to_str": { tf: 3.872983346207417 },
            "scouter.FeatureAlerts.model_dump_json": { tf: 3.872983346207417 },
            "scouter.Every30Minutes.get_next": { tf: 3.872983346207417 },
            "scouter.EveryHour.get_next": { tf: 3.872983346207417 },
            "scouter.Every6Hours.get_next": { tf: 3.872983346207417 },
            "scouter.Every12Hours.get_next": { tf: 3.872983346207417 },
            "scouter.EveryDay.get_next": { tf: 3.872983346207417 },
            "scouter.EveryWeek.get_next": { tf: 3.872983346207417 },
            "scouter.DriftMap.add_feature": { tf: 4.795831523312719 },
            "scouter.DriftMap.model_dump_json": { tf: 3.872983346207417 },
            "scouter.DriftMap.load_from_json": { tf: 3.1622776601683795 },
            "scouter.DriftMap.save_to_json": { tf: 4.795831523312719 },
            "scouter.DriftMap.to_server_record": { tf: 3.872983346207417 },
            "scouter.DriftMap.to_numpy": { tf: 3.872983346207417 },
          },
          df: 26,
          s: {
            docs: {},
            df: 0,
            e: {
              docs: {},
              df: 0,
              l: {
                docs: {},
                df: 0,
                f: {
                  docs: {
                    "scouter.Profiler.create_data_profile": { tf: 1 },
                    "scouter.Drifter.create_drift_profile": { tf: 1 },
                    "scouter.Drifter.compute_drift": { tf: 1 },
                    "scouter.Drifter.generate_alerts": { tf: 1 },
                    "scouter.DataProfile.model_dump_json": { tf: 1 },
                    "scouter.DataProfile.save_to_json": { tf: 1 },
                    "scouter.DriftProfile.model_dump_json": { tf: 1 },
                    "scouter.DriftProfile.save_to_json": { tf: 1 },
                    "scouter.AlertRule.to_str": { tf: 1 },
                    "scouter.FeatureAlerts.model_dump_json": { tf: 1 },
                    "scouter.Every30Minutes.get_next": { tf: 1 },
                    "scouter.EveryHour.get_next": { tf: 1 },
                    "scouter.Every6Hours.get_next": { tf: 1 },
                    "scouter.Every12Hours.get_next": { tf: 1 },
                    "scouter.EveryDay.get_next": { tf: 1 },
                    "scouter.EveryWeek.get_next": { tf: 1 },
                    "scouter.DriftMap.add_feature": { tf: 1 },
                    "scouter.DriftMap.model_dump_json": { tf: 1 },
                    "scouter.DriftMap.save_to_json": { tf: 1 },
                    "scouter.DriftMap.to_server_record": { tf: 1 },
                    "scouter.DriftMap.to_numpy": { tf: 1 },
                  },
                  df: 21,
                },
              },
            },
            c: {
              docs: {},
              df: 0,
              a: {
                docs: {},
                df: 0,
                l: {
                  docs: {},
                  df: 0,
                  a: {
                    docs: {},
                    df: 0,
                    r: {
                      docs: {},
                      df: 0,
                      t: {
                        docs: {},
                        df: 0,
                        y: {
                          docs: {},
                          df: 0,
                          p: {
                            docs: {},
                            df: 0,
                            e: {
                              docs: {
                                "scouter.Profiler.create_data_profile": {
                                  tf: 1,
                                },
                                "scouter.Drifter.create_drift_profile": {
                                  tf: 1,
                                },
                                "scouter.Drifter.compute_drift": { tf: 1 },
                                "scouter.Drifter.generate_alerts": { tf: 1 },
                              },
                              df: 4,
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            t: {
              docs: {},
              df: 0,
              r: {
                docs: {
                  "scouter.Profiler.create_data_profile": { tf: 1 },
                  "scouter.Drifter.create_drift_profile": { tf: 1 },
                  "scouter.Drifter.compute_drift": { tf: 1 },
                  "scouter.Drifter.generate_alerts": { tf: 1 },
                },
                df: 4,
              },
            },
            i: {
              docs: {},
              df: 0,
              z: {
                docs: {},
                df: 0,
                e: {
                  docs: { "scouter.Profiler.create_data_profile": { tf: 1 } },
                  df: 1,
                },
              },
            },
          },
          d: {
            docs: {},
            df: 0,
            a: {
              docs: {},
              df: 0,
              t: {
                docs: {},
                df: 0,
                a: {
                  docs: {
                    "scouter.Profiler.create_data_profile": { tf: 1 },
                    "scouter.Drifter.create_drift_profile": { tf: 1 },
                    "scouter.Drifter.compute_drift": { tf: 1 },
                  },
                  df: 3,
                  f: {
                    docs: {},
                    df: 0,
                    r: {
                      docs: {},
                      df: 0,
                      a: {
                        docs: {},
                        df: 0,
                        m: {
                          docs: {},
                          df: 0,
                          e: {
                            docs: {
                              "scouter.Profiler.create_data_profile": {
                                tf: 1.7320508075688772,
                              },
                              "scouter.Drifter.create_drift_profile": {
                                tf: 1.7320508075688772,
                              },
                              "scouter.Drifter.compute_drift": {
                                tf: 1.7320508075688772,
                              },
                            },
                            df: 3,
                          },
                        },
                      },
                    },
                  },
                  p: {
                    docs: {},
                    df: 0,
                    r: {
                      docs: {},
                      df: 0,
                      o: {
                        docs: {},
                        df: 0,
                        f: {
                          docs: {},
                          df: 0,
                          i: {
                            docs: {},
                            df: 0,
                            l: {
                              docs: {},
                              df: 0,
                              e: {
                                docs: {
                                  "scouter.Profiler.create_data_profile": {
                                    tf: 1,
                                  },
                                },
                                df: 1,
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            t: {
              docs: {},
              df: 0,
              y: {
                docs: {},
                df: 0,
                p: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {
                      "scouter.Profiler.create_data_profile": { tf: 1 },
                      "scouter.Drifter.create_drift_profile": { tf: 1 },
                      "scouter.Drifter.compute_drift": { tf: 1 },
                      "scouter.Drifter.generate_alerts": { tf: 1 },
                    },
                    df: 4,
                  },
                },
              },
            },
            r: {
              docs: {},
              df: 0,
              i: {
                docs: {},
                df: 0,
                f: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {
                      "scouter.Drifter.compute_drift": { tf: 1 },
                      "scouter.Drifter.generate_alerts": { tf: 1 },
                    },
                    df: 2,
                    c: {
                      docs: {},
                      df: 0,
                      o: {
                        docs: {},
                        df: 0,
                        n: {
                          docs: {},
                          df: 0,
                          f: {
                            docs: {},
                            df: 0,
                            i: {
                              docs: {},
                              df: 0,
                              g: {
                                docs: {
                                  "scouter.Drifter.create_drift_profile": {
                                    tf: 1,
                                  },
                                },
                                df: 1,
                              },
                            },
                          },
                        },
                      },
                    },
                    p: {
                      docs: {},
                      df: 0,
                      r: {
                        docs: {},
                        df: 0,
                        o: {
                          docs: {},
                          df: 0,
                          f: {
                            docs: {},
                            df: 0,
                            i: {
                              docs: {},
                              df: 0,
                              l: {
                                docs: {},
                                df: 0,
                                e: {
                                  docs: {
                                    "scouter.Drifter.create_drift_profile": {
                                      tf: 1,
                                    },
                                    "scouter.Drifter.compute_drift": { tf: 1 },
                                  },
                                  df: 2,
                                },
                              },
                            },
                          },
                        },
                      },
                    },
                    m: {
                      docs: {},
                      df: 0,
                      a: {
                        docs: {},
                        df: 0,
                        p: {
                          docs: { "scouter.Drifter.compute_drift": { tf: 1 } },
                          df: 1,
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          u: {
            docs: {},
            df: 0,
            n: {
              docs: {},
              df: 0,
              i: {
                docs: {},
                df: 0,
                o: {
                  docs: {},
                  df: 0,
                  n: {
                    docs: {
                      "scouter.Profiler.create_data_profile": { tf: 1 },
                      "scouter.Drifter.create_drift_profile": { tf: 1 },
                      "scouter.Drifter.compute_drift": { tf: 1 },
                    },
                    df: 3,
                  },
                },
              },
            },
          },
          p: {
            docs: {},
            df: 0,
            o: {
              docs: {},
              df: 0,
              l: {
                docs: {},
                df: 0,
                a: {
                  docs: {},
                  df: 0,
                  r: {
                    docs: {},
                    df: 0,
                    s: {
                      docs: {
                        "scouter.Profiler.create_data_profile": { tf: 1 },
                        "scouter.Drifter.create_drift_profile": { tf: 1 },
                        "scouter.Drifter.compute_drift": { tf: 1 },
                      },
                      df: 3,
                    },
                  },
                },
              },
            },
            a: {
              docs: {},
              df: 0,
              n: {
                docs: {},
                df: 0,
                d: {
                  docs: {},
                  df: 0,
                  a: {
                    docs: {},
                    df: 0,
                    s: {
                      docs: {
                        "scouter.Profiler.create_data_profile": { tf: 1 },
                        "scouter.Drifter.create_drift_profile": { tf: 1 },
                        "scouter.Drifter.compute_drift": { tf: 1 },
                      },
                      df: 3,
                    },
                  },
                },
              },
              t: {
                docs: {},
                df: 0,
                h: {
                  docs: {
                    "scouter.DataProfile.save_to_json": { tf: 1 },
                    "scouter.DriftProfile.save_to_json": { tf: 1 },
                    "scouter.DriftMap.save_to_json": { tf: 1 },
                  },
                  df: 3,
                },
              },
            },
            r: {
              docs: {},
              df: 0,
              o: {
                docs: {},
                df: 0,
                f: {
                  docs: {},
                  df: 0,
                  i: {
                    docs: {},
                    df: 0,
                    l: {
                      docs: {},
                      df: 0,
                      e: {
                        docs: {
                          "scouter.Drifter.compute_drift": { tf: 1 },
                          "scouter.DriftMap.add_feature": { tf: 1 },
                        },
                        df: 2,
                      },
                    },
                  },
                },
              },
            },
          },
          f: {
            docs: {},
            df: 0,
            r: {
              docs: {},
              df: 0,
              a: {
                docs: {},
                df: 0,
                m: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {
                      "scouter.Profiler.create_data_profile": {
                        tf: 1.4142135623730951,
                      },
                      "scouter.Drifter.create_drift_profile": {
                        tf: 1.4142135623730951,
                      },
                      "scouter.Drifter.compute_drift": {
                        tf: 1.4142135623730951,
                      },
                    },
                    df: 3,
                  },
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              a: {
                docs: {},
                df: 0,
                t: {
                  docs: {},
                  df: 0,
                  u: {
                    docs: {},
                    df: 0,
                    r: {
                      docs: {},
                      df: 0,
                      e: {
                        docs: { "scouter.DriftMap.add_feature": { tf: 1 } },
                        df: 1,
                        s: {
                          docs: {
                            "scouter.Profiler.create_data_profile": { tf: 1 },
                            "scouter.Drifter.create_drift_profile": { tf: 1 },
                            "scouter.Drifter.compute_drift": { tf: 1 },
                            "scouter.Drifter.generate_alerts": { tf: 1 },
                          },
                          df: 4,
                        },
                        a: {
                          docs: {},
                          df: 0,
                          l: {
                            docs: {},
                            df: 0,
                            e: {
                              docs: {},
                              df: 0,
                              r: {
                                docs: {},
                                df: 0,
                                t: {
                                  docs: {},
                                  df: 0,
                                  s: {
                                    docs: {
                                      "scouter.Drifter.generate_alerts": {
                                        tf: 1,
                                      },
                                    },
                                    df: 1,
                                  },
                                },
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          c: {
            docs: {},
            df: 0,
            o: {
              docs: {
                "scouter.Profiler.create_data_profile": { tf: 1 },
                "scouter.Drifter.create_drift_profile": { tf: 1 },
                "scouter.Drifter.compute_drift": { tf: 1 },
                "scouter.Drifter.generate_alerts": { tf: 1 },
              },
              df: 4,
              r: {
                docs: {},
                df: 0,
                e: {
                  docs: {
                    "scouter.Profiler.create_data_profile": { tf: 1 },
                    "scouter.Drifter.create_drift_profile": { tf: 1 },
                    "scouter.Drifter.compute_drift": { tf: 1 },
                  },
                  df: 3,
                },
              },
              n: {
                docs: {},
                df: 0,
                f: {
                  docs: {},
                  df: 0,
                  i: {
                    docs: {},
                    df: 0,
                    g: {
                      docs: {
                        "scouter.Drifter.create_drift_profile": { tf: 1 },
                      },
                      df: 1,
                    },
                  },
                },
              },
            },
          },
          n: {
            docs: {},
            df: 0,
            u: {
              docs: {},
              df: 0,
              m: {
                docs: {},
                df: 0,
                p: {
                  docs: {},
                  df: 0,
                  y: {
                    docs: {
                      "scouter.Profiler.create_data_profile": {
                        tf: 1.4142135623730951,
                      },
                      "scouter.Drifter.create_drift_profile": {
                        tf: 1.4142135623730951,
                      },
                      "scouter.Drifter.compute_drift": {
                        tf: 1.4142135623730951,
                      },
                      "scouter.Drifter.generate_alerts": {
                        tf: 1.4142135623730951,
                      },
                    },
                    df: 4,
                  },
                },
              },
            },
            d: {
              docs: {},
              df: 0,
              a: {
                docs: {},
                df: 0,
                r: {
                  docs: {},
                  df: 0,
                  r: {
                    docs: {},
                    df: 0,
                    a: {
                      docs: {},
                      df: 0,
                      y: {
                        docs: {
                          "scouter.Profiler.create_data_profile": { tf: 1 },
                          "scouter.Drifter.create_drift_profile": { tf: 1 },
                          "scouter.Drifter.compute_drift": { tf: 1 },
                          "scouter.Drifter.generate_alerts": { tf: 1 },
                        },
                        df: 4,
                      },
                    },
                  },
                },
              },
            },
            o: {
              docs: {},
              df: 0,
              n: {
                docs: {},
                df: 0,
                e: {
                  docs: {
                    "scouter.Profiler.create_data_profile": { tf: 1 },
                    "scouter.Drifter.create_drift_profile": { tf: 1 },
                    "scouter.Drifter.compute_drift": { tf: 1 },
                    "scouter.DataProfile.save_to_json": { tf: 1 },
                    "scouter.DriftProfile.save_to_json": { tf: 1 },
                    "scouter.DriftMap.save_to_json": { tf: 1 },
                  },
                  df: 6,
                },
              },
            },
          },
          a: {
            docs: {},
            df: 0,
            n: {
              docs: {},
              df: 0,
              y: {
                docs: {
                  "scouter.Profiler.create_data_profile": { tf: 1 },
                  "scouter.Drifter.create_drift_profile": { tf: 1 },
                  "scouter.Drifter.compute_drift": { tf: 1 },
                  "scouter.Drifter.generate_alerts": { tf: 1 },
                },
                df: 4,
              },
            },
            r: {
              docs: {},
              df: 0,
              r: {
                docs: {},
                df: 0,
                a: {
                  docs: {},
                  df: 0,
                  y: {
                    docs: { "scouter.Drifter.generate_alerts": { tf: 1 } },
                    df: 1,
                  },
                },
              },
            },
            l: {
              docs: {},
              df: 0,
              e: {
                docs: {},
                df: 0,
                r: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: { "scouter.Drifter.generate_alerts": { tf: 1 } },
                    df: 1,
                    r: {
                      docs: {},
                      df: 0,
                      u: {
                        docs: {},
                        df: 0,
                        l: {
                          docs: {},
                          df: 0,
                          e: {
                            docs: {
                              "scouter.Drifter.generate_alerts": { tf: 1 },
                            },
                            df: 1,
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          o: {
            docs: {},
            df: 0,
            p: {
              docs: {},
              df: 0,
              t: {
                docs: {},
                df: 0,
                i: {
                  docs: {},
                  df: 0,
                  o: {
                    docs: {},
                    df: 0,
                    n: {
                      docs: {},
                      df: 0,
                      a: {
                        docs: {},
                        df: 0,
                        l: {
                          docs: {
                            "scouter.Profiler.create_data_profile": { tf: 1 },
                            "scouter.Drifter.create_drift_profile": { tf: 1 },
                            "scouter.Drifter.compute_drift": { tf: 1 },
                          },
                          df: 3,
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          l: {
            docs: {},
            df: 0,
            i: {
              docs: {},
              df: 0,
              s: {
                docs: {},
                df: 0,
                t: {
                  docs: {
                    "scouter.Profiler.create_data_profile": { tf: 1 },
                    "scouter.Drifter.create_drift_profile": { tf: 1 },
                    "scouter.Drifter.compute_drift": { tf: 1 },
                    "scouter.Drifter.generate_alerts": { tf: 1 },
                  },
                  df: 4,
                },
              },
            },
          },
          b: {
            docs: {},
            df: 0,
            i: {
              docs: {},
              df: 0,
              n: {
                docs: { "scouter.Profiler.create_data_profile": { tf: 1 } },
                df: 1,
              },
            },
          },
          i: {
            docs: {},
            df: 0,
            n: {
              docs: {},
              df: 0,
              t: {
                docs: { "scouter.Profiler.create_data_profile": { tf: 1 } },
                df: 1,
              },
            },
          },
          m: {
            docs: {},
            df: 0,
            o: {
              docs: {},
              df: 0,
              n: {
                docs: {},
                df: 0,
                i: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {},
                    df: 0,
                    o: {
                      docs: {},
                      df: 0,
                      r: {
                        docs: {
                          "scouter.Drifter.create_drift_profile": { tf: 1 },
                        },
                        df: 1,
                      },
                    },
                  },
                },
              },
              d: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  l: {
                    docs: {
                      "scouter.DataProfile.load_from_json": { tf: 1 },
                      "scouter.DriftProfile.load_from_json": { tf: 1 },
                      "scouter.DriftMap.load_from_json": { tf: 1 },
                    },
                    df: 3,
                  },
                },
              },
            },
          },
          t: {
            docs: {},
            df: 0,
            y: {
              docs: {},
              df: 0,
              p: {
                docs: {},
                df: 0,
                i: {
                  docs: {},
                  df: 0,
                  n: {
                    docs: {},
                    df: 0,
                    g: {
                      docs: { "scouter.Drifter.generate_alerts": { tf: 1 } },
                      df: 1,
                    },
                  },
                },
              },
            },
          },
          r: {
            docs: {},
            df: 0,
            u: {
              docs: {},
              df: 0,
              l: {
                docs: {},
                df: 0,
                e: {
                  docs: { "scouter.Drifter.generate_alerts": { tf: 1 } },
                  df: 1,
                },
              },
            },
          },
        },
      },
      bases: {
        root: {
          docs: {},
          df: 0,
          s: {
            docs: {},
            df: 0,
            c: {
              docs: {},
              df: 0,
              o: {
                docs: {},
                df: 0,
                u: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {},
                    df: 0,
                    e: {
                      docs: {},
                      df: 0,
                      r: {
                        docs: {
                          "scouter.Profiler": { tf: 1.4142135623730951 },
                          "scouter.Drifter": { tf: 1.4142135623730951 },
                        },
                        df: 2,
                        b: {
                          docs: {},
                          df: 0,
                          a: {
                            docs: {},
                            df: 0,
                            s: {
                              docs: {},
                              df: 0,
                              e: {
                                docs: {
                                  "scouter.Profiler": { tf: 1 },
                                  "scouter.Drifter": { tf: 1 },
                                },
                                df: 2,
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            t: {
              docs: {},
              df: 0,
              r: {
                docs: {
                  "scouter.AlertType": { tf: 1 },
                  "scouter.AlertZone": { tf: 1 },
                },
                df: 2,
              },
            },
          },
          b: {
            docs: {},
            df: 0,
            u: {
              docs: {},
              df: 0,
              i: {
                docs: {},
                df: 0,
                l: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {},
                    df: 0,
                    i: {
                      docs: {},
                      df: 0,
                      n: {
                        docs: {},
                        df: 0,
                        s: {
                          docs: {
                            "scouter.AlertType": { tf: 1 },
                            "scouter.AlertZone": { tf: 1 },
                          },
                          df: 2,
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          e: {
            docs: {},
            df: 0,
            n: {
              docs: {},
              df: 0,
              u: {
                docs: {},
                df: 0,
                m: {
                  docs: {
                    "scouter.AlertType": { tf: 1.4142135623730951 },
                    "scouter.AlertZone": { tf: 1.4142135623730951 },
                  },
                  df: 2,
                },
              },
            },
          },
        },
      },
      doc: {
        root: {
          2: {
            0: {
              docs: { "scouter.Profiler.create_data_profile": { tf: 1 } },
              df: 1,
            },
            docs: {},
            df: 0,
          },
          docs: {
            scouter: { tf: 1.7320508075688772 },
            "scouter.Profiler": { tf: 1.7320508075688772 },
            "scouter.Profiler.__init__": { tf: 1.7320508075688772 },
            "scouter.Profiler.create_data_profile": { tf: 5.656854249492381 },
            "scouter.Drifter": { tf: 1.7320508075688772 },
            "scouter.Drifter.__init__": { tf: 1.4142135623730951 },
            "scouter.Drifter.create_drift_profile": { tf: 5.656854249492381 },
            "scouter.Drifter.compute_drift": { tf: 4.898979485566356 },
            "scouter.Drifter.generate_alerts": { tf: 5.744562646538029 },
            "scouter.DataProfile": { tf: 1.7320508075688772 },
            "scouter.DataProfile.model_dump_json": { tf: 1.7320508075688772 },
            "scouter.DataProfile.load_from_json": { tf: 1.7320508075688772 },
            "scouter.DataProfile.save_to_json": { tf: 1.7320508075688772 },
            "scouter.DataProfile.features": { tf: 1.7320508075688772 },
            "scouter.DriftProfile": { tf: 1.7320508075688772 },
            "scouter.DriftProfile.model_dump_json": { tf: 1.7320508075688772 },
            "scouter.DriftProfile.load_from_json": { tf: 1.7320508075688772 },
            "scouter.DriftProfile.save_to_json": { tf: 1.7320508075688772 },
            "scouter.DriftProfile.features": { tf: 1.7320508075688772 },
            "scouter.DriftProfile.config": { tf: 1.7320508075688772 },
            "scouter.FeatureDriftProfile": { tf: 5.291502622129181 },
            "scouter.FeatureDriftProfile.two_lcl": { tf: 1.7320508075688772 },
            "scouter.FeatureDriftProfile.two_ucl": { tf: 1.7320508075688772 },
            "scouter.FeatureDriftProfile.one_ucl": { tf: 1.7320508075688772 },
            "scouter.FeatureDriftProfile.three_ucl": { tf: 1.7320508075688772 },
            "scouter.FeatureDriftProfile.center": { tf: 1.7320508075688772 },
            "scouter.FeatureDriftProfile.id": { tf: 1.7320508075688772 },
            "scouter.FeatureDriftProfile.one_lcl": { tf: 1.7320508075688772 },
            "scouter.FeatureDriftProfile.three_lcl": { tf: 1.7320508075688772 },
            "scouter.FeatureDriftProfile.timestamp": { tf: 1.7320508075688772 },
            "scouter.FeatureProfile": { tf: 1.7320508075688772 },
            "scouter.FeatureProfile.quantiles": { tf: 1.7320508075688772 },
            "scouter.FeatureProfile.distinct": { tf: 1.7320508075688772 },
            "scouter.FeatureProfile.stddev": { tf: 1.7320508075688772 },
            "scouter.FeatureProfile.timestamp": { tf: 1.7320508075688772 },
            "scouter.FeatureProfile.histogram": { tf: 1.7320508075688772 },
            "scouter.FeatureProfile.id": { tf: 1.7320508075688772 },
            "scouter.FeatureProfile.mean": { tf: 1.7320508075688772 },
            "scouter.FeatureProfile.max": { tf: 1.7320508075688772 },
            "scouter.FeatureProfile.min": { tf: 1.7320508075688772 },
            "scouter.Alert": { tf: 1.7320508075688772 },
            "scouter.Alert.kind": { tf: 1.7320508075688772 },
            "scouter.Alert.zone": { tf: 1.7320508075688772 },
            "scouter.AlertType": { tf: 2.6457513110645907 },
            "scouter.AlertType.OutOfBounds": { tf: 1.7320508075688772 },
            "scouter.AlertType.Consecutive": { tf: 1.7320508075688772 },
            "scouter.AlertType.Alternating": { tf: 1.7320508075688772 },
            "scouter.AlertType.AllGood": { tf: 1.7320508075688772 },
            "scouter.AlertType.Trend": { tf: 1.7320508075688772 },
            "scouter.AlertRule": { tf: 1.7320508075688772 },
            "scouter.AlertRule.to_str": { tf: 1.7320508075688772 },
            "scouter.AlertRule.process": { tf: 1.7320508075688772 },
            "scouter.AlertRule.percentage": { tf: 1.7320508075688772 },
            "scouter.AlertZone": { tf: 2.6457513110645907 },
            "scouter.AlertZone.Zone1": { tf: 1.7320508075688772 },
            "scouter.AlertZone.Zone2": { tf: 1.7320508075688772 },
            "scouter.AlertZone.Zone3": { tf: 1.7320508075688772 },
            "scouter.AlertZone.OutOfBounds": { tf: 1.7320508075688772 },
            "scouter.AlertZone.NotApplicable": { tf: 1.7320508075688772 },
            "scouter.FeatureAlerts": { tf: 1.7320508075688772 },
            "scouter.FeatureAlerts.model_dump_json": { tf: 1.7320508075688772 },
            "scouter.FeatureAlerts.features": { tf: 1.7320508075688772 },
            "scouter.ProcessAlertRule": { tf: 1.7320508075688772 },
            "scouter.ProcessAlertRule.rule": { tf: 1.7320508075688772 },
            "scouter.PercentageAlertRule": { tf: 1.7320508075688772 },
            "scouter.PercentageAlertRule.rule": { tf: 1.7320508075688772 },
            "scouter.CommonCrons": { tf: 1.7320508075688772 },
            "scouter.CommonCron": { tf: 1.7320508075688772 },
            "scouter.CommonCron.every_day": { tf: 1.7320508075688772 },
            "scouter.CommonCron.every_week": { tf: 1.7320508075688772 },
            "scouter.CommonCron.every_hour": { tf: 1.7320508075688772 },
            "scouter.CommonCron.every_6_hours": { tf: 1.7320508075688772 },
            "scouter.CommonCron.every_30_minutes": { tf: 1.7320508075688772 },
            "scouter.CommonCron.every_12_hours": { tf: 1.7320508075688772 },
            "scouter.Every30Minutes": { tf: 1.7320508075688772 },
            "scouter.Every30Minutes.get_next": { tf: 1.7320508075688772 },
            "scouter.Every30Minutes.cron": { tf: 1.7320508075688772 },
            "scouter.EveryHour": { tf: 1.7320508075688772 },
            "scouter.EveryHour.get_next": { tf: 1.7320508075688772 },
            "scouter.EveryHour.cron": { tf: 1.7320508075688772 },
            "scouter.Every6Hours": { tf: 1.7320508075688772 },
            "scouter.Every6Hours.get_next": { tf: 1.7320508075688772 },
            "scouter.Every6Hours.cron": { tf: 1.7320508075688772 },
            "scouter.Every12Hours": { tf: 1.7320508075688772 },
            "scouter.Every12Hours.get_next": { tf: 1.7320508075688772 },
            "scouter.Every12Hours.cron": { tf: 1.7320508075688772 },
            "scouter.EveryDay": { tf: 1.7320508075688772 },
            "scouter.EveryDay.get_next": { tf: 1.7320508075688772 },
            "scouter.EveryDay.cron": { tf: 1.7320508075688772 },
            "scouter.EveryWeek": { tf: 1.7320508075688772 },
            "scouter.EveryWeek.get_next": { tf: 1.7320508075688772 },
            "scouter.EveryWeek.cron": { tf: 1.7320508075688772 },
            "scouter.DriftConfig": { tf: 6 },
            "scouter.DriftConfig.sample_size": { tf: 1.7320508075688772 },
            "scouter.DriftConfig.name": { tf: 1.7320508075688772 },
            "scouter.DriftConfig.sample": { tf: 1.7320508075688772 },
            "scouter.DriftConfig.version": { tf: 1.7320508075688772 },
            "scouter.DriftConfig.schedule": { tf: 1.7320508075688772 },
            "scouter.DriftConfig.repository": { tf: 1.7320508075688772 },
            "scouter.DriftConfig.alert_rule": { tf: 1.7320508075688772 },
            "scouter.DriftMap": { tf: 3.4641016151377544 },
            "scouter.DriftMap.add_feature": { tf: 1.7320508075688772 },
            "scouter.DriftMap.model_dump_json": { tf: 1.7320508075688772 },
            "scouter.DriftMap.load_from_json": { tf: 1.7320508075688772 },
            "scouter.DriftMap.save_to_json": { tf: 1.7320508075688772 },
            "scouter.DriftMap.to_server_record": { tf: 1.7320508075688772 },
            "scouter.DriftMap.to_numpy": { tf: 1.7320508075688772 },
            "scouter.DriftMap.name": { tf: 1.7320508075688772 },
            "scouter.DriftMap.repository": { tf: 1.7320508075688772 },
            "scouter.DriftMap.version": { tf: 1.7320508075688772 },
            "scouter.DriftMap.features": { tf: 1.7320508075688772 },
          },
          df: 111,
          s: {
            docs: {},
            df: 0,
            c: {
              docs: {},
              df: 0,
              o: {
                docs: {},
                df: 0,
                u: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {},
                    df: 0,
                    e: {
                      docs: {},
                      df: 0,
                      r: {
                        docs: {
                          "scouter.Profiler.__init__": { tf: 1 },
                          "scouter.Drifter.__init__": { tf: 1 },
                        },
                        df: 2,
                      },
                    },
                  },
                },
              },
              h: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  d: {
                    docs: {},
                    df: 0,
                    u: {
                      docs: {},
                      df: 0,
                      l: {
                        docs: {},
                        df: 0,
                        e: {
                          docs: {
                            "scouter.DriftConfig": { tf: 1.4142135623730951 },
                          },
                          df: 1,
                        },
                      },
                    },
                  },
                },
              },
            },
            t: {
              docs: {},
              df: 0,
              a: {
                docs: {},
                df: 0,
                t: {
                  docs: {},
                  df: 0,
                  i: {
                    docs: {},
                    df: 0,
                    s: {
                      docs: {},
                      df: 0,
                      t: {
                        docs: {},
                        df: 0,
                        i: {
                          docs: {},
                          df: 0,
                          c: {
                            docs: {},
                            df: 0,
                            s: {
                              docs: { "scouter.Profiler.__init__": { tf: 1 } },
                              df: 1,
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
              r: {
                docs: {
                  "scouter.AlertType": { tf: 2.23606797749979 },
                  "scouter.AlertZone": { tf: 2.23606797749979 },
                },
                df: 2,
                i: {
                  docs: {},
                  df: 0,
                  n: {
                    docs: {},
                    df: 0,
                    g: {
                      docs: {
                        "scouter.AlertType": { tf: 1 },
                        "scouter.AlertZone": { tf: 1 },
                      },
                      df: 2,
                    },
                  },
                  c: {
                    docs: {},
                    df: 0,
                    t: {
                      docs: {
                        "scouter.AlertType": { tf: 1 },
                        "scouter.AlertZone": { tf: 1 },
                      },
                      df: 2,
                    },
                  },
                },
              },
            },
            i: {
              docs: {},
              df: 0,
              z: {
                docs: {},
                df: 0,
                e: {
                  docs: {
                    "scouter.Profiler.create_data_profile": {
                      tf: 1.4142135623730951,
                    },
                    "scouter.DriftConfig": { tf: 1.4142135623730951 },
                  },
                  df: 2,
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              t: {
                docs: {},
                df: 0,
                u: {
                  docs: {},
                  df: 0,
                  p: { docs: { "scouter.Drifter.__init__": { tf: 1 } }, df: 1 },
                },
              },
            },
            p: {
              docs: {},
              df: 0,
              e: {
                docs: {},
                df: 0,
                c: {
                  docs: {},
                  df: 0,
                  i: {
                    docs: {},
                    df: 0,
                    f: {
                      docs: {},
                      df: 0,
                      i: {
                        docs: {},
                        df: 0,
                        e: {
                          docs: {},
                          df: 0,
                          d: {
                            docs: {
                              "scouter.AlertType": { tf: 1 },
                              "scouter.AlertZone": { tf: 1 },
                            },
                            df: 2,
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            y: {
              docs: {},
              df: 0,
              s: {
                docs: {
                  "scouter.AlertType": { tf: 1 },
                  "scouter.AlertZone": { tf: 1 },
                },
                df: 2,
              },
            },
            a: {
              docs: {},
              df: 0,
              m: {
                docs: {},
                df: 0,
                p: {
                  docs: {},
                  df: 0,
                  l: {
                    docs: {},
                    df: 0,
                    e: { docs: { "scouter.DriftConfig": { tf: 2 } }, df: 1 },
                  },
                },
              },
            },
          },
          c: {
            docs: {},
            df: 0,
            l: {
              docs: {},
              df: 0,
              a: {
                docs: {},
                df: 0,
                s: {
                  docs: {},
                  df: 0,
                  s: {
                    docs: {
                      "scouter.Profiler.__init__": { tf: 1.4142135623730951 },
                      "scouter.Drifter.__init__": { tf: 1.7320508075688772 },
                      "scouter.FeatureDriftProfile": { tf: 1 },
                      "scouter.DriftConfig": { tf: 1 },
                      "scouter.DriftMap": { tf: 1 },
                    },
                    df: 5,
                  },
                },
              },
            },
            r: {
              docs: {},
              df: 0,
              e: {
                docs: {},
                df: 0,
                a: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {},
                    df: 0,
                    i: {
                      docs: {},
                      df: 0,
                      n: {
                        docs: {},
                        df: 0,
                        g: {
                          docs: {
                            "scouter.Profiler.__init__": { tf: 1 },
                            "scouter.Drifter.__init__": { tf: 1 },
                          },
                          df: 2,
                        },
                      },
                    },
                    e: {
                      docs: {
                        "scouter.Profiler.create_data_profile": {
                          tf: 1.4142135623730951,
                        },
                        "scouter.Drifter.__init__": { tf: 1 },
                        "scouter.Drifter.create_drift_profile": {
                          tf: 1.4142135623730951,
                        },
                        "scouter.AlertType": { tf: 1 },
                        "scouter.AlertZone": { tf: 1 },
                      },
                      df: 5,
                      d: {
                        docs: {
                          "scouter.Profiler.create_data_profile": { tf: 1 },
                          "scouter.Drifter.create_drift_profile": { tf: 1 },
                        },
                        df: 2,
                      },
                    },
                  },
                },
              },
              o: {
                docs: {},
                df: 0,
                n: { docs: { "scouter.DriftConfig": { tf: 1 } }, df: 1 },
              },
            },
            a: {
              docs: {},
              df: 0,
              n: {
                docs: {
                  "scouter.Profiler.create_data_profile": { tf: 1 },
                  "scouter.Drifter.create_drift_profile": { tf: 1 },
                  "scouter.Drifter.compute_drift": { tf: 1 },
                },
                df: 3,
              },
              l: {
                docs: {},
                df: 0,
                c: {
                  docs: {},
                  df: 0,
                  u: {
                    docs: {},
                    df: 0,
                    l: {
                      docs: {},
                      df: 0,
                      a: {
                        docs: {},
                        df: 0,
                        t: {
                          docs: {},
                          df: 0,
                          e: {
                            docs: {},
                            df: 0,
                            d: {
                              docs: { "scouter.DriftMap": { tf: 1 } },
                              df: 1,
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            o: {
              docs: {},
              df: 0,
              n: {
                docs: {},
                df: 0,
                t: {
                  docs: {},
                  df: 0,
                  a: {
                    docs: {},
                    df: 0,
                    i: {
                      docs: {},
                      df: 0,
                      n: {
                        docs: {
                          "scouter.Profiler.create_data_profile": { tf: 1 },
                          "scouter.Drifter.create_drift_profile": { tf: 1 },
                          "scouter.Drifter.compute_drift": { tf: 1 },
                        },
                        df: 3,
                        i: {
                          docs: {},
                          df: 0,
                          n: {
                            docs: {},
                            df: 0,
                            g: {
                              docs: {
                                "scouter.Drifter.compute_drift": { tf: 1 },
                              },
                              df: 1,
                            },
                          },
                        },
                      },
                    },
                  },
                  r: {
                    docs: {},
                    df: 0,
                    o: {
                      docs: {},
                      df: 0,
                      l: {
                        docs: {
                          "scouter.FeatureDriftProfile": {
                            tf: 1.4142135623730951,
                          },
                        },
                        df: 1,
                      },
                    },
                  },
                },
                f: {
                  docs: {},
                  df: 0,
                  i: {
                    docs: {},
                    df: 0,
                    g: {
                      docs: {
                        "scouter.Drifter.create_drift_profile": { tf: 1 },
                      },
                      df: 1,
                      u: {
                        docs: {},
                        df: 0,
                        r: {
                          docs: {},
                          df: 0,
                          a: {
                            docs: {},
                            df: 0,
                            t: {
                              docs: {},
                              df: 0,
                              i: {
                                docs: {},
                                df: 0,
                                o: {
                                  docs: {},
                                  df: 0,
                                  n: {
                                    docs: {
                                      "scouter.Drifter.create_drift_profile": {
                                        tf: 1,
                                      },
                                      "scouter.DriftConfig": { tf: 1 },
                                    },
                                    df: 2,
                                  },
                                },
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
              m: {
                docs: {},
                df: 0,
                p: {
                  docs: {},
                  df: 0,
                  u: {
                    docs: {},
                    df: 0,
                    t: {
                      docs: {},
                      df: 0,
                      e: {
                        docs: {
                          "scouter.Drifter.compute_drift": {
                            tf: 1.4142135623730951,
                          },
                        },
                        df: 1,
                      },
                    },
                  },
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              n: {
                docs: {},
                df: 0,
                t: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {},
                    df: 0,
                    r: {
                      docs: {
                        "scouter.FeatureDriftProfile": {
                          tf: 1.4142135623730951,
                        },
                      },
                      df: 1,
                    },
                  },
                },
              },
            },
          },
          f: {
            docs: {},
            df: 0,
            o: {
              docs: {},
              df: 0,
              r: {
                docs: {
                  "scouter.Profiler.__init__": { tf: 1.4142135623730951 },
                  "scouter.Profiler.create_data_profile": { tf: 1 },
                  "scouter.Drifter.__init__": { tf: 1 },
                  "scouter.Drifter.create_drift_profile": {
                    tf: 1.4142135623730951,
                  },
                  "scouter.FeatureDriftProfile": { tf: 1 },
                  "scouter.DriftConfig": { tf: 1.7320508075688772 },
                  "scouter.DriftMap": { tf: 1 },
                },
                df: 7,
              },
            },
            r: {
              docs: {},
              df: 0,
              o: {
                docs: {},
                df: 0,
                m: {
                  docs: {
                    "scouter.Profiler.create_data_profile": {
                      tf: 1.4142135623730951,
                    },
                    "scouter.Drifter.__init__": { tf: 1.4142135623730951 },
                    "scouter.Drifter.create_drift_profile": {
                      tf: 1.4142135623730951,
                    },
                    "scouter.Drifter.compute_drift": { tf: 1.4142135623730951 },
                    "scouter.Drifter.generate_alerts": { tf: 1 },
                    "scouter.AlertType": { tf: 1 },
                    "scouter.AlertZone": { tf: 1 },
                  },
                  df: 7,
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              a: {
                docs: {},
                df: 0,
                t: {
                  docs: {},
                  df: 0,
                  u: {
                    docs: {},
                    df: 0,
                    r: {
                      docs: {},
                      df: 0,
                      e: {
                        docs: {
                          "scouter.Profiler.create_data_profile": {
                            tf: 1.4142135623730951,
                          },
                          "scouter.Drifter.create_drift_profile": {
                            tf: 1.4142135623730951,
                          },
                          "scouter.Drifter.compute_drift": { tf: 2 },
                          "scouter.Drifter.generate_alerts": { tf: 1 },
                          "scouter.DriftMap": { tf: 1 },
                        },
                        df: 5,
                        s: {
                          docs: {
                            "scouter.Profiler.create_data_profile": { tf: 1 },
                            "scouter.Drifter.create_drift_profile": { tf: 1 },
                            "scouter.Drifter.compute_drift": { tf: 1 },
                            "scouter.Drifter.generate_alerts": {
                              tf: 1.4142135623730951,
                            },
                            "scouter.DriftMap": { tf: 1.4142135623730951 },
                          },
                          df: 5,
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          d: {
            docs: {},
            df: 0,
            a: {
              docs: {},
              df: 0,
              t: {
                docs: {},
                df: 0,
                a: {
                  docs: {
                    "scouter.Profiler.__init__": { tf: 1 },
                    "scouter.Profiler.create_data_profile": {
                      tf: 2.8284271247461903,
                    },
                    "scouter.Drifter.__init__": { tf: 1.4142135623730951 },
                    "scouter.Drifter.create_drift_profile": {
                      tf: 2.23606797749979,
                    },
                    "scouter.Drifter.compute_drift": { tf: 2.23606797749979 },
                    "scouter.AlertType": { tf: 1 },
                    "scouter.AlertZone": { tf: 1 },
                    "scouter.DriftConfig": { tf: 1 },
                  },
                  df: 8,
                  s: {
                    docs: {},
                    df: 0,
                    e: {
                      docs: {},
                      df: 0,
                      t: {
                        docs: {
                          "scouter.Profiler.__init__": { tf: 1 },
                          "scouter.Drifter.__init__": { tf: 1 },
                        },
                        df: 2,
                      },
                    },
                  },
                  f: {
                    docs: {},
                    df: 0,
                    r: {
                      docs: {},
                      df: 0,
                      a: {
                        docs: {},
                        df: 0,
                        m: {
                          docs: {},
                          df: 0,
                          e: {
                            docs: {
                              "scouter.Profiler.create_data_profile": {
                                tf: 1.4142135623730951,
                              },
                              "scouter.Drifter.create_drift_profile": {
                                tf: 1.4142135623730951,
                              },
                              "scouter.Drifter.compute_drift": {
                                tf: 1.4142135623730951,
                              },
                            },
                            df: 3,
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              f: {
                docs: {},
                df: 0,
                a: {
                  docs: {},
                  df: 0,
                  u: {
                    docs: {},
                    df: 0,
                    l: {
                      docs: {},
                      df: 0,
                      t: {
                        docs: { "scouter.DriftConfig": { tf: 1 } },
                        df: 1,
                        s: {
                          docs: {
                            "scouter.Profiler.create_data_profile": { tf: 1 },
                            "scouter.AlertType": { tf: 1.4142135623730951 },
                            "scouter.AlertZone": { tf: 1.4142135623730951 },
                          },
                          df: 3,
                        },
                      },
                    },
                  },
                },
                i: {
                  docs: {},
                  df: 0,
                  n: {
                    docs: {},
                    df: 0,
                    e: {
                      docs: {},
                      df: 0,
                      d: {
                        docs: {
                          "scouter.AlertType": { tf: 1 },
                          "scouter.AlertZone": { tf: 1 },
                        },
                        df: 2,
                      },
                    },
                  },
                },
              },
              t: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  c: {
                    docs: {},
                    df: 0,
                    t: {
                      docs: { "scouter.Drifter.__init__": { tf: 1 } },
                      df: 1,
                      i: {
                        docs: {},
                        df: 0,
                        n: {
                          docs: {},
                          df: 0,
                          g: {
                            docs: { "scouter.Drifter.__init__": { tf: 1 } },
                            df: 1,
                          },
                        },
                      },
                    },
                  },
                },
              },
              c: {
                docs: {},
                df: 0,
                o: {
                  docs: {},
                  df: 0,
                  d: {
                    docs: {},
                    df: 0,
                    e: {
                      docs: {},
                      df: 0,
                      d: {
                        docs: {
                          "scouter.AlertType": { tf: 1 },
                          "scouter.AlertZone": { tf: 1 },
                        },
                        df: 2,
                      },
                    },
                  },
                },
              },
            },
            r: {
              docs: {},
              df: 0,
              i: {
                docs: {},
                df: 0,
                f: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {
                      "scouter.Drifter.__init__": { tf: 1.7320508075688772 },
                      "scouter.Drifter.create_drift_profile": { tf: 1 },
                      "scouter.Drifter.compute_drift": { tf: 2 },
                      "scouter.Drifter.generate_alerts": {
                        tf: 2.23606797749979,
                      },
                      "scouter.DriftMap": { tf: 1.7320508075688772 },
                    },
                    df: 5,
                  },
                },
              },
            },
            i: {
              docs: {},
              df: 0,
              c: {
                docs: {},
                df: 0,
                t: {
                  docs: {},
                  df: 0,
                  i: {
                    docs: {},
                    df: 0,
                    o: {
                      docs: {},
                      df: 0,
                      n: {
                        docs: {},
                        df: 0,
                        a: {
                          docs: {},
                          df: 0,
                          r: {
                            docs: {},
                            df: 0,
                            y: {
                              docs: {
                                "scouter.Drifter.generate_alerts": { tf: 1 },
                              },
                              df: 1,
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          p: {
            docs: {},
            df: 0,
            r: {
              docs: {},
              df: 0,
              o: {
                docs: {},
                df: 0,
                f: {
                  docs: {},
                  df: 0,
                  i: {
                    docs: {},
                    df: 0,
                    l: {
                      docs: {},
                      df: 0,
                      e: {
                        docs: {
                          "scouter.Profiler.create_data_profile": { tf: 2 },
                          "scouter.Drifter.__init__": { tf: 1 },
                          "scouter.Drifter.create_drift_profile": {
                            tf: 2.23606797749979,
                          },
                          "scouter.Drifter.compute_drift": { tf: 2 },
                          "scouter.FeatureDriftProfile": { tf: 1 },
                        },
                        df: 5,
                        s: {
                          docs: {
                            "scouter.Profiler.__init__": { tf: 1 },
                            "scouter.Drifter.__init__": { tf: 1 },
                            "scouter.Drifter.compute_drift": { tf: 1 },
                          },
                          df: 3,
                        },
                      },
                    },
                  },
                },
                v: {
                  docs: {},
                  df: 0,
                  i: {
                    docs: {},
                    df: 0,
                    d: {
                      docs: {},
                      df: 0,
                      e: {
                        docs: {},
                        df: 0,
                        d: {
                          docs: {
                            "scouter.Profiler.create_data_profile": { tf: 1 },
                            "scouter.Drifter.create_drift_profile": { tf: 1 },
                            "scouter.Drifter.compute_drift": { tf: 1 },
                          },
                          df: 3,
                        },
                      },
                    },
                  },
                },
              },
              e: {
                docs: {},
                df: 0,
                s: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {},
                    df: 0,
                    n: {
                      docs: {},
                      df: 0,
                      t: {
                        docs: {
                          "scouter.Profiler.create_data_profile": { tf: 1 },
                          "scouter.Drifter.create_drift_profile": { tf: 1 },
                        },
                        df: 2,
                      },
                    },
                  },
                },
              },
              i: {
                docs: {},
                df: 0,
                m: {
                  docs: {},
                  df: 0,
                  a: {
                    docs: {},
                    df: 0,
                    r: {
                      docs: {},
                      df: 0,
                      i: {
                        docs: {},
                        df: 0,
                        l: {
                          docs: {},
                          df: 0,
                          y: {
                            docs: { "scouter.Drifter.__init__": { tf: 1 } },
                            df: 1,
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            o: {
              docs: {},
              df: 0,
              l: {
                docs: {},
                df: 0,
                a: {
                  docs: {},
                  df: 0,
                  r: {
                    docs: {},
                    df: 0,
                    s: {
                      docs: {
                        "scouter.Profiler.create_data_profile": { tf: 1 },
                        "scouter.Drifter.create_drift_profile": { tf: 1 },
                        "scouter.Drifter.compute_drift": { tf: 1 },
                      },
                      df: 3,
                    },
                  },
                },
              },
            },
            a: {
              docs: {},
              df: 0,
              n: {
                docs: {},
                df: 0,
                d: {
                  docs: {},
                  df: 0,
                  a: {
                    docs: {},
                    df: 0,
                    s: {
                      docs: {
                        "scouter.Profiler.create_data_profile": { tf: 1 },
                        "scouter.Drifter.create_drift_profile": { tf: 1 },
                        "scouter.Drifter.compute_drift": { tf: 1 },
                      },
                      df: 3,
                    },
                  },
                },
              },
            },
            y: {
              docs: {},
              df: 0,
              t: {
                docs: {},
                df: 0,
                h: {
                  docs: {},
                  df: 0,
                  o: {
                    docs: {},
                    df: 0,
                    n: {
                      docs: {
                        "scouter.FeatureDriftProfile": { tf: 1 },
                        "scouter.DriftConfig": { tf: 1 },
                        "scouter.DriftMap": { tf: 1 },
                      },
                      df: 3,
                    },
                  },
                },
              },
            },
          },
          t: {
            docs: {},
            df: 0,
            h: {
              docs: {},
              df: 0,
              i: {
                docs: {},
                df: 0,
                s: {
                  docs: {
                    "scouter.Profiler.__init__": { tf: 1 },
                    "scouter.Drifter.__init__": { tf: 1.4142135623730951 },
                  },
                  df: 2,
                },
              },
              e: {
                docs: {
                  "scouter.Profiler.create_data_profile": { tf: 1 },
                  "scouter.Drifter.create_drift_profile": {
                    tf: 1.4142135623730951,
                  },
                  "scouter.Drifter.compute_drift": { tf: 1.4142135623730951 },
                  "scouter.Drifter.generate_alerts": { tf: 1.4142135623730951 },
                  "scouter.FeatureDriftProfile": { tf: 2.23606797749979 },
                  "scouter.AlertType": { tf: 2 },
                  "scouter.AlertZone": { tf: 2 },
                  "scouter.DriftConfig": { tf: 3 },
                },
                df: 8,
                s: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {
                      "scouter.Profiler.create_data_profile": { tf: 1 },
                      "scouter.Drifter.create_drift_profile": { tf: 1 },
                    },
                    df: 2,
                  },
                },
                n: {
                  docs: {
                    "scouter.AlertType": { tf: 1 },
                    "scouter.AlertZone": { tf: 1 },
                  },
                  df: 2,
                },
                i: {
                  docs: {},
                  df: 0,
                  r: { docs: { "scouter.DriftMap": { tf: 1 } }, df: 1 },
                },
              },
              a: {
                docs: {},
                df: 0,
                t: {
                  docs: {
                    "scouter.AlertType": { tf: 1 },
                    "scouter.AlertZone": { tf: 1 },
                  },
                  df: 2,
                },
              },
            },
            o: {
              docs: {
                "scouter.Profiler.create_data_profile": {
                  tf: 1.7320508075688772,
                },
                "scouter.Drifter.__init__": { tf: 1 },
                "scouter.Drifter.create_drift_profile": {
                  tf: 1.7320508075688772,
                },
                "scouter.Drifter.compute_drift": { tf: 1.4142135623730951 },
                "scouter.Drifter.generate_alerts": { tf: 1.4142135623730951 },
                "scouter.AlertType": { tf: 1.4142135623730951 },
                "scouter.AlertZone": { tf: 1.4142135623730951 },
                "scouter.DriftConfig": { tf: 1.4142135623730951 },
              },
              df: 8,
            },
            i: {
              docs: {},
              df: 0,
              m: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  s: {
                    docs: {},
                    df: 0,
                    t: {
                      docs: {},
                      df: 0,
                      a: {
                        docs: {},
                        df: 0,
                        m: {
                          docs: {},
                          df: 0,
                          p: {
                            docs: {
                              "scouter.FeatureDriftProfile": {
                                tf: 1.4142135623730951,
                              },
                            },
                            df: 1,
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            r: {
              docs: {},
              df: 0,
              u: {
                docs: {},
                df: 0,
                e: { docs: { "scouter.DriftConfig": { tf: 1 } }, df: 1 },
              },
            },
          },
          w: {
            docs: {},
            df: 0,
            i: {
              docs: {},
              df: 0,
              l: {
                docs: {},
                df: 0,
                l: {
                  docs: {
                    "scouter.Profiler.__init__": { tf: 1 },
                    "scouter.Profiler.create_data_profile": {
                      tf: 1.4142135623730951,
                    },
                    "scouter.Drifter.__init__": { tf: 1 },
                    "scouter.Drifter.create_drift_profile": {
                      tf: 1.4142135623730951,
                    },
                    "scouter.Drifter.compute_drift": { tf: 1 },
                    "scouter.AlertType": { tf: 1 },
                    "scouter.AlertZone": { tf: 1 },
                  },
                  df: 7,
                },
              },
              t: {
                docs: {},
                df: 0,
                h: {
                  docs: {
                    "scouter.DriftConfig": { tf: 1 },
                    "scouter.DriftMap": { tf: 1 },
                  },
                  df: 2,
                },
              },
            },
            h: {
              docs: {},
              df: 0,
              e: {
                docs: {},
                df: 0,
                t: {
                  docs: {},
                  df: 0,
                  h: {
                    docs: {},
                    df: 0,
                    e: {
                      docs: {},
                      df: 0,
                      r: { docs: { "scouter.DriftConfig": { tf: 1 } }, df: 1 },
                    },
                  },
                },
              },
            },
          },
          g: {
            docs: {},
            df: 0,
            e: {
              docs: {},
              df: 0,
              n: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  r: {
                    docs: {},
                    df: 0,
                    a: {
                      docs: {},
                      df: 0,
                      t: {
                        docs: {},
                        df: 0,
                        e: {
                          docs: {
                            "scouter.Profiler.__init__": { tf: 1 },
                            "scouter.Drifter.generate_alerts": { tf: 1 },
                          },
                          df: 2,
                          d: {
                            docs: {
                              "scouter.Profiler.create_data_profile": { tf: 1 },
                              "scouter.Drifter.create_drift_profile": { tf: 1 },
                              "scouter.Drifter.compute_drift": { tf: 1 },
                            },
                            df: 3,
                          },
                        },
                      },
                    },
                  },
                },
              },
              t: {
                docs: {},
                df: 0,
                d: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {},
                    df: 0,
                    f: {
                      docs: {},
                      df: 0,
                      a: {
                        docs: {},
                        df: 0,
                        u: {
                          docs: {},
                          df: 0,
                          l: {
                            docs: {},
                            df: 0,
                            t: {
                              docs: {},
                              df: 0,
                              e: {
                                docs: {},
                                df: 0,
                                n: {
                                  docs: {},
                                  df: 0,
                                  c: {
                                    docs: {},
                                    df: 0,
                                    o: {
                                      docs: {},
                                      df: 0,
                                      d: {
                                        docs: {},
                                        df: 0,
                                        i: {
                                          docs: {},
                                          df: 0,
                                          n: {
                                            docs: {},
                                            df: 0,
                                            g: {
                                              docs: {
                                                "scouter.AlertType": { tf: 1 },
                                                "scouter.AlertZone": { tf: 1 },
                                              },
                                              df: 2,
                                            },
                                          },
                                        },
                                      },
                                    },
                                  },
                                },
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            i: {
              docs: {},
              df: 0,
              v: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  n: {
                    docs: {
                      "scouter.Profiler.__init__": { tf: 1 },
                      "scouter.AlertType": { tf: 1.4142135623730951 },
                      "scouter.AlertZone": { tf: 1.4142135623730951 },
                    },
                    df: 3,
                  },
                },
              },
            },
          },
          b: {
            docs: {},
            df: 0,
            a: {
              docs: {},
              df: 0,
              s: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  l: {
                    docs: {},
                    df: 0,
                    i: {
                      docs: {},
                      df: 0,
                      n: {
                        docs: {},
                        df: 0,
                        e: {
                          docs: { "scouter.Profiler.__init__": { tf: 1 } },
                          df: 1,
                        },
                      },
                    },
                  },
                },
              },
            },
            e: {
              docs: {
                "scouter.Profiler.create_data_profile": { tf: 2 },
                "scouter.Drifter.create_drift_profile": { tf: 2 },
                "scouter.Drifter.compute_drift": { tf: 1.4142135623730951 },
                "scouter.AlertType": { tf: 1 },
                "scouter.AlertZone": { tf: 1 },
              },
              df: 5,
            },
            i: {
              docs: {},
              df: 0,
              n: {
                docs: {
                  "scouter.Profiler.create_data_profile": {
                    tf: 1.4142135623730951,
                  },
                },
                df: 1,
                s: {
                  docs: { "scouter.Profiler.create_data_profile": { tf: 1 } },
                  df: 1,
                },
              },
            },
            y: {
              docs: {},
              df: 0,
              t: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  s: {
                    docs: {
                      "scouter.AlertType": { tf: 1 },
                      "scouter.AlertZone": { tf: 1 },
                    },
                    df: 2,
                  },
                },
              },
            },
            u: {
              docs: {},
              df: 0,
              f: {
                docs: {},
                df: 0,
                f: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {},
                    df: 0,
                    r: {
                      docs: {
                        "scouter.AlertType": { tf: 1.4142135623730951 },
                        "scouter.AlertZone": { tf: 1.4142135623730951 },
                      },
                      df: 2,
                    },
                  },
                },
              },
            },
          },
          a: {
            docs: {
              "scouter.Profiler.__init__": { tf: 1 },
              "scouter.Profiler.create_data_profile": { tf: 2 },
              "scouter.Drifter.__init__": { tf: 1.4142135623730951 },
              "scouter.Drifter.create_drift_profile": { tf: 2 },
              "scouter.Drifter.compute_drift": { tf: 1.4142135623730951 },
              "scouter.Drifter.generate_alerts": { tf: 1 },
              "scouter.FeatureDriftProfile": { tf: 1 },
              "scouter.AlertType": { tf: 1.4142135623730951 },
              "scouter.AlertZone": { tf: 1.4142135623730951 },
              "scouter.DriftConfig": { tf: 1 },
              "scouter.DriftMap": { tf: 1.4142135623730951 },
            },
            df: 11,
            r: {
              docs: {},
              df: 0,
              g: {
                docs: {},
                df: 0,
                u: {
                  docs: {},
                  df: 0,
                  m: {
                    docs: {},
                    df: 0,
                    e: {
                      docs: {},
                      df: 0,
                      n: {
                        docs: {},
                        df: 0,
                        t: {
                          docs: {},
                          df: 0,
                          s: {
                            docs: {
                              "scouter.Profiler.create_data_profile": { tf: 1 },
                              "scouter.Drifter.create_drift_profile": { tf: 1 },
                              "scouter.Drifter.compute_drift": { tf: 1 },
                              "scouter.Drifter.generate_alerts": { tf: 1 },
                              "scouter.FeatureDriftProfile": { tf: 1 },
                              "scouter.DriftConfig": { tf: 1 },
                              "scouter.DriftMap": { tf: 1 },
                            },
                            df: 7,
                          },
                        },
                      },
                    },
                  },
                },
              },
              r: {
                docs: {},
                df: 0,
                a: {
                  docs: {},
                  df: 0,
                  y: {
                    docs: {
                      "scouter.Profiler.create_data_profile": { tf: 1 },
                      "scouter.Drifter.create_drift_profile": { tf: 1 },
                      "scouter.Drifter.compute_drift": { tf: 1 },
                      "scouter.Drifter.generate_alerts": { tf: 2 },
                    },
                    df: 4,
                  },
                },
              },
              e: {
                docs: {
                  "scouter.Profiler.create_data_profile": { tf: 1 },
                  "scouter.Drifter.create_drift_profile": { tf: 1 },
                },
                df: 2,
              },
            },
            u: {
              docs: {},
              df: 0,
              t: {
                docs: {},
                df: 0,
                o: {
                  docs: {},
                  df: 0,
                  m: {
                    docs: {},
                    df: 0,
                    a: {
                      docs: {},
                      df: 0,
                      t: {
                        docs: {},
                        df: 0,
                        i: {
                          docs: {},
                          df: 0,
                          c: {
                            docs: {},
                            df: 0,
                            a: {
                              docs: {},
                              df: 0,
                              l: {
                                docs: {},
                                df: 0,
                                l: {
                                  docs: {},
                                  df: 0,
                                  y: {
                                    docs: {
                                      "scouter.Profiler.create_data_profile": {
                                        tf: 1,
                                      },
                                      "scouter.Drifter.create_drift_profile": {
                                        tf: 1,
                                      },
                                      "scouter.Drifter.compute_drift": {
                                        tf: 1,
                                      },
                                    },
                                    df: 3,
                                  },
                                },
                              },
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            n: {
              docs: {},
              df: 0,
              y: {
                docs: {
                  "scouter.Profiler.create_data_profile": { tf: 1 },
                  "scouter.Drifter.create_drift_profile": { tf: 1 },
                  "scouter.Drifter.compute_drift": { tf: 1 },
                },
                df: 3,
              },
              d: {
                docs: {
                  "scouter.Drifter.__init__": { tf: 1.7320508075688772 },
                  "scouter.Drifter.compute_drift": { tf: 1 },
                  "scouter.Drifter.generate_alerts": { tf: 1 },
                  "scouter.AlertType": { tf: 1 },
                  "scouter.AlertZone": { tf: 1 },
                  "scouter.DriftMap": { tf: 1 },
                },
                df: 6,
              },
            },
            c: {
              docs: {},
              df: 0,
              t: {
                docs: {},
                df: 0,
                i: {
                  docs: {},
                  df: 0,
                  v: {
                    docs: {},
                    df: 0,
                    e: {
                      docs: {},
                      df: 0,
                      l: {
                        docs: {},
                        df: 0,
                        y: {
                          docs: { "scouter.Drifter.__init__": { tf: 1 } },
                          df: 1,
                        },
                      },
                    },
                  },
                },
              },
            },
            l: {
              docs: {},
              df: 0,
              e: {
                docs: {},
                df: 0,
                r: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {
                      "scouter.Drifter.generate_alerts": {
                        tf: 1.4142135623730951,
                      },
                      "scouter.DriftConfig": { tf: 1 },
                    },
                    df: 2,
                    s: {
                      docs: {
                        "scouter.Drifter.generate_alerts": {
                          tf: 1.4142135623730951,
                        },
                      },
                      df: 1,
                    },
                    i: {
                      docs: {},
                      df: 0,
                      n: {
                        docs: {},
                        df: 0,
                        g: {
                          docs: { "scouter.DriftConfig": { tf: 1 } },
                          df: 1,
                        },
                      },
                    },
                  },
                },
              },
            },
            p: {
              docs: {},
              df: 0,
              p: {
                docs: {},
                df: 0,
                l: {
                  docs: {},
                  df: 0,
                  y: {
                    docs: { "scouter.Drifter.generate_alerts": { tf: 1 } },
                    df: 1,
                  },
                },
              },
            },
            s: {
              docs: {},
              df: 0,
              s: {
                docs: {},
                df: 0,
                o: {
                  docs: {},
                  df: 0,
                  c: {
                    docs: {},
                    df: 0,
                    i: {
                      docs: {},
                      df: 0,
                      a: {
                        docs: {},
                        df: 0,
                        t: {
                          docs: {},
                          df: 0,
                          e: {
                            docs: {},
                            df: 0,
                            d: {
                              docs: { "scouter.DriftConfig": { tf: 1 } },
                              df: 1,
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          o: {
            docs: {},
            df: 0,
            p: {
              docs: {},
              df: 0,
              t: {
                docs: {},
                df: 0,
                i: {
                  docs: {},
                  df: 0,
                  o: {
                    docs: {},
                    df: 0,
                    n: {
                      docs: {},
                      df: 0,
                      a: {
                        docs: {},
                        df: 0,
                        l: {
                          docs: {
                            "scouter.Profiler.create_data_profile": {
                              tf: 1.4142135623730951,
                            },
                            "scouter.Drifter.create_drift_profile": { tf: 1 },
                            "scouter.Drifter.compute_drift": { tf: 1 },
                          },
                          df: 3,
                        },
                      },
                    },
                  },
                },
              },
            },
            f: {
              docs: {
                "scouter.Profiler.create_data_profile": { tf: 1 },
                "scouter.Drifter.create_drift_profile": { tf: 1 },
                "scouter.Drifter.compute_drift": { tf: 1 },
                "scouter.Drifter.generate_alerts": { tf: 2 },
                "scouter.AlertType": { tf: 1 },
                "scouter.AlertZone": { tf: 1 },
                "scouter.DriftConfig": { tf: 1.4142135623730951 },
                "scouter.DriftMap": { tf: 1.4142135623730951 },
              },
              df: 8,
            },
            r: {
              docs: {
                "scouter.Profiler.create_data_profile": { tf: 2 },
                "scouter.Drifter.create_drift_profile": { tf: 2 },
                "scouter.Drifter.compute_drift": { tf: 1.4142135623730951 },
                "scouter.AlertType": { tf: 1.7320508075688772 },
                "scouter.AlertZone": { tf: 1.7320508075688772 },
                "scouter.DriftConfig": { tf: 1 },
              },
              df: 6,
              d: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  r: {
                    docs: { "scouter.Drifter.generate_alerts": { tf: 1 } },
                    df: 1,
                  },
                },
              },
            },
            b: {
              docs: {},
              df: 0,
              j: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  c: {
                    docs: {},
                    df: 0,
                    t: {
                      docs: {
                        "scouter.AlertType": { tf: 2.449489742783178 },
                        "scouter.AlertZone": { tf: 2.449489742783178 },
                      },
                      df: 2,
                    },
                  },
                },
              },
            },
            t: {
              docs: {},
              df: 0,
              h: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  r: {
                    docs: {},
                    df: 0,
                    w: {
                      docs: {},
                      df: 0,
                      i: {
                        docs: {},
                        df: 0,
                        s: {
                          docs: {},
                          df: 0,
                          e: {
                            docs: {
                              "scouter.AlertType": { tf: 1 },
                              "scouter.AlertZone": { tf: 1 },
                            },
                            df: 2,
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
          },
          l: {
            docs: {},
            df: 0,
            i: {
              docs: {},
              df: 0,
              s: {
                docs: {},
                df: 0,
                t: {
                  docs: {
                    "scouter.Profiler.create_data_profile": { tf: 1 },
                    "scouter.Drifter.create_drift_profile": { tf: 1 },
                    "scouter.Drifter.compute_drift": { tf: 1 },
                    "scouter.Drifter.generate_alerts": { tf: 1 },
                  },
                  df: 4,
                },
              },
              m: {
                docs: {},
                df: 0,
                i: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {
                      "scouter.FeatureDriftProfile": { tf: 1.4142135623730951 },
                    },
                    df: 1,
                  },
                },
              },
            },
            c: {
              docs: {},
              df: 0,
              l: { docs: { "scouter.FeatureDriftProfile": { tf: 1 } }, df: 1 },
            },
            o: {
              docs: {},
              df: 0,
              w: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  r: {
                    docs: { "scouter.FeatureDriftProfile": { tf: 1 } },
                    df: 1,
                  },
                },
              },
            },
          },
          n: {
            docs: {},
            df: 0,
            a: {
              docs: {},
              df: 0,
              m: {
                docs: {},
                df: 0,
                e: {
                  docs: { "scouter.DriftConfig": { tf: 1.4142135623730951 } },
                  df: 1,
                  s: {
                    docs: {
                      "scouter.Profiler.create_data_profile": {
                        tf: 1.4142135623730951,
                      },
                      "scouter.Drifter.create_drift_profile": {
                        tf: 1.4142135623730951,
                      },
                      "scouter.Drifter.compute_drift": { tf: 2 },
                      "scouter.Drifter.generate_alerts": { tf: 1 },
                      "scouter.DriftMap": { tf: 1 },
                    },
                    df: 5,
                  },
                },
              },
              n: {
                docs: {},
                df: 0,
                s: {
                  docs: {
                    "scouter.Profiler.create_data_profile": {
                      tf: 1.4142135623730951,
                    },
                    "scouter.Drifter.create_drift_profile": {
                      tf: 1.4142135623730951,
                    },
                    "scouter.Drifter.compute_drift": { tf: 1 },
                  },
                  df: 3,
                },
              },
            },
            o: {
              docs: {},
              df: 0,
              t: {
                docs: {
                  "scouter.Profiler.create_data_profile": {
                    tf: 1.7320508075688772,
                  },
                  "scouter.Drifter.create_drift_profile": {
                    tf: 1.7320508075688772,
                  },
                  "scouter.Drifter.compute_drift": { tf: 1.4142135623730951 },
                  "scouter.DriftConfig": { tf: 1 },
                },
                df: 4,
              },
            },
            u: {
              docs: {},
              df: 0,
              m: {
                docs: {},
                df: 0,
                p: {
                  docs: {},
                  df: 0,
                  y: {
                    docs: {
                      "scouter.Profiler.create_data_profile": { tf: 1 },
                      "scouter.Drifter.create_drift_profile": { tf: 1 },
                      "scouter.Drifter.compute_drift": { tf: 1 },
                    },
                    df: 3,
                  },
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              w: {
                docs: {
                  "scouter.Drifter.__init__": { tf: 1 },
                  "scouter.AlertType": { tf: 1 },
                  "scouter.AlertZone": { tf: 1 },
                },
                df: 3,
              },
            },
          },
          i: {
            docs: {},
            df: 0,
            f: {
              docs: {
                "scouter.Profiler.create_data_profile": {
                  tf: 1.4142135623730951,
                },
                "scouter.Drifter.create_drift_profile": {
                  tf: 1.4142135623730951,
                },
                "scouter.Drifter.compute_drift": { tf: 1 },
                "scouter.AlertType": { tf: 1.4142135623730951 },
                "scouter.AlertZone": { tf: 1.4142135623730951 },
              },
              df: 5,
            },
            s: {
              docs: {
                "scouter.Profiler.create_data_profile": { tf: 1 },
                "scouter.Drifter.__init__": { tf: 1 },
                "scouter.Drifter.create_drift_profile": { tf: 1 },
                "scouter.Drifter.compute_drift": { tf: 1 },
                "scouter.AlertType": { tf: 1 },
                "scouter.AlertZone": { tf: 1 },
                "scouter.DriftConfig": { tf: 1 },
              },
              df: 7,
            },
            n: {
              docs: { "scouter.Drifter.compute_drift": { tf: 1 } },
              df: 1,
              f: {
                docs: {},
                df: 0,
                i: {
                  docs: {},
                  df: 0,
                  n: {
                    docs: {},
                    df: 0,
                    i: {
                      docs: {},
                      df: 0,
                      t: {
                        docs: {},
                        df: 0,
                        i: {
                          docs: {},
                          df: 0,
                          e: {
                            docs: {},
                            df: 0,
                            s: {
                              docs: {
                                "scouter.Profiler.create_data_profile": {
                                  tf: 1.4142135623730951,
                                },
                                "scouter.Drifter.create_drift_profile": {
                                  tf: 1.4142135623730951,
                                },
                                "scouter.Drifter.compute_drift": { tf: 1 },
                              },
                              df: 3,
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            m: {
              docs: {},
              df: 0,
              p: {
                docs: {},
                df: 0,
                u: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {},
                    df: 0,
                    e: {
                      docs: {},
                      df: 0,
                      d: {
                        docs: {
                          "scouter.Profiler.create_data_profile": { tf: 1 },
                          "scouter.Drifter.create_drift_profile": { tf: 1 },
                        },
                        df: 2,
                      },
                    },
                  },
                },
              },
            },
            d: {
              docs: {
                "scouter.FeatureDriftProfile": { tf: 1.4142135623730951 },
              },
              df: 1,
            },
          },
          e: {
            docs: {},
            df: 0,
            x: {
              docs: {},
              df: 0,
              p: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  c: {
                    docs: {},
                    df: 0,
                    t: {
                      docs: {},
                      df: 0,
                      e: {
                        docs: {},
                        df: 0,
                        d: {
                          docs: {
                            "scouter.Profiler.create_data_profile": { tf: 1 },
                            "scouter.Drifter.create_drift_profile": { tf: 1 },
                            "scouter.Drifter.compute_drift": { tf: 1 },
                          },
                          df: 3,
                        },
                      },
                    },
                  },
                },
                o: {
                  docs: {},
                  df: 0,
                  s: {
                    docs: {},
                    df: 0,
                    e: {
                      docs: {
                        "scouter.AlertType": { tf: 1 },
                        "scouter.AlertZone": { tf: 1 },
                      },
                      df: 2,
                    },
                  },
                },
              },
            },
            n: {
              docs: {},
              df: 0,
              c: {
                docs: {},
                df: 0,
                o: {
                  docs: {},
                  df: 0,
                  d: {
                    docs: {},
                    df: 0,
                    i: {
                      docs: {},
                      df: 0,
                      n: {
                        docs: {},
                        df: 0,
                        g: {
                          docs: {
                            "scouter.AlertType": { tf: 2 },
                            "scouter.AlertZone": { tf: 2 },
                          },
                          df: 2,
                        },
                      },
                    },
                  },
                },
              },
            },
            r: {
              docs: {},
              df: 0,
              r: {
                docs: {},
                df: 0,
                o: {
                  docs: {},
                  df: 0,
                  r: {
                    docs: {
                      "scouter.AlertType": { tf: 1 },
                      "scouter.AlertZone": { tf: 1 },
                    },
                    df: 2,
                    s: {
                      docs: {
                        "scouter.AlertType": { tf: 1.7320508075688772 },
                        "scouter.AlertZone": { tf: 1.7320508075688772 },
                      },
                      df: 2,
                    },
                  },
                },
              },
            },
          },
          m: {
            docs: {},
            df: 0,
            i: {
              docs: {},
              df: 0,
              s: {
                docs: {},
                df: 0,
                s: {
                  docs: {},
                  df: 0,
                  i: {
                    docs: {},
                    df: 0,
                    n: {
                      docs: {},
                      df: 0,
                      g: {
                        docs: {
                          "scouter.Profiler.create_data_profile": { tf: 1 },
                          "scouter.Drifter.create_drift_profile": { tf: 1 },
                          "scouter.Drifter.compute_drift": { tf: 1 },
                        },
                        df: 3,
                      },
                    },
                  },
                },
              },
            },
            u: {
              docs: {},
              df: 0,
              s: {
                docs: {},
                df: 0,
                t: {
                  docs: {
                    "scouter.Profiler.create_data_profile": { tf: 1 },
                    "scouter.Drifter.create_drift_profile": { tf: 1 },
                    "scouter.Drifter.compute_drift": { tf: 1 },
                    "scouter.Drifter.generate_alerts": { tf: 1 },
                    "scouter.AlertType": { tf: 1 },
                    "scouter.AlertZone": { tf: 1 },
                  },
                  df: 6,
                },
              },
            },
            o: {
              docs: {},
              df: 0,
              n: {
                docs: {},
                df: 0,
                i: {
                  docs: {},
                  df: 0,
                  t: {
                    docs: {},
                    df: 0,
                    o: {
                      docs: {},
                      df: 0,
                      r: {
                        docs: {
                          "scouter.Drifter.__init__": { tf: 1 },
                          "scouter.Drifter.create_drift_profile": { tf: 1 },
                        },
                        df: 2,
                        i: {
                          docs: {},
                          df: 0,
                          n: {
                            docs: {},
                            df: 0,
                            g: {
                              docs: {
                                "scouter.Profiler.create_data_profile": {
                                  tf: 1,
                                },
                                "scouter.Drifter.__init__": {
                                  tf: 1.4142135623730951,
                                },
                                "scouter.Drifter.create_drift_profile": {
                                  tf: 2.23606797749979,
                                },
                                "scouter.Drifter.compute_drift": {
                                  tf: 1.7320508075688772,
                                },
                                "scouter.FeatureDriftProfile": { tf: 1 },
                                "scouter.DriftConfig": {
                                  tf: 1.7320508075688772,
                                },
                              },
                              df: 6,
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
              d: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  l: {
                    docs: { "scouter.DriftConfig": { tf: 1.7320508075688772 } },
                    df: 1,
                  },
                },
              },
            },
            a: {
              docs: {},
              df: 0,
              t: {
                docs: {},
                df: 0,
                c: {
                  docs: {},
                  df: 0,
                  h: {
                    docs: {
                      "scouter.Drifter.compute_drift": { tf: 1 },
                      "scouter.Drifter.generate_alerts": { tf: 1 },
                    },
                    df: 2,
                  },
                },
              },
              p: { docs: { "scouter.DriftMap": { tf: 1 } }, df: 1 },
            },
          },
          v: {
            docs: {},
            df: 0,
            a: {
              docs: {},
              df: 0,
              l: {
                docs: {},
                df: 0,
                u: {
                  docs: {},
                  df: 0,
                  e: {
                    docs: {
                      "scouter.FeatureDriftProfile": { tf: 1.7320508075688772 },
                    },
                    df: 1,
                    s: {
                      docs: {
                        "scouter.Profiler.create_data_profile": {
                          tf: 1.4142135623730951,
                        },
                        "scouter.Drifter.create_drift_profile": {
                          tf: 1.4142135623730951,
                        },
                        "scouter.Drifter.compute_drift": { tf: 1 },
                        "scouter.Drifter.generate_alerts": {
                          tf: 1.4142135623730951,
                        },
                      },
                      df: 4,
                    },
                  },
                },
              },
            },
            e: {
              docs: {},
              df: 0,
              r: {
                docs: {},
                df: 0,
                s: {
                  docs: {},
                  df: 0,
                  i: {
                    docs: {},
                    df: 0,
                    o: {
                      docs: {},
                      df: 0,
                      n: {
                        docs: {
                          "scouter.DriftConfig": { tf: 1.4142135623730951 },
                        },
                        df: 1,
                      },
                    },
                  },
                },
              },
            },
          },
          r: {
            docs: {},
            df: 0,
            e: {
              docs: {},
              df: 0,
              m: {
                docs: {},
                df: 0,
                o: {
                  docs: {},
                  df: 0,
                  v: {
                    docs: {},
                    df: 0,
                    e: {
                      docs: {},
                      df: 0,
                      d: {
                        docs: {
                          "scouter.Profiler.create_data_profile": { tf: 1 },
                          "scouter.Drifter.create_drift_profile": { tf: 1 },
                        },
                        df: 2,
                      },
                    },
                  },
                },
              },
              t: {
                docs: {},
                df: 0,
                u: {
                  docs: {},
                  df: 0,
                  r: {
                    docs: {},
                    df: 0,
                    n: {
                      docs: {},
                      df: 0,
                      s: {
                        docs: {
                          "scouter.Profiler.create_data_profile": { tf: 1 },
                          "scouter.Drifter.create_drift_profile": { tf: 1 },
                          "scouter.Drifter.generate_alerts": { tf: 1 },
                          "scouter.AlertType": { tf: 1 },
                          "scouter.AlertZone": { tf: 1 },
                        },
                        df: 5,
                      },
                    },
                  },
                },
              },
              s: {
                docs: {},
                df: 0,
                u: {
                  docs: {},
                  df: 0,
                  l: {
                    docs: {},
                    df: 0,
                    t: {
                      docs: {
                        "scouter.AlertType": { tf: 1 },
                        "scouter.AlertZone": { tf: 1 },
                      },
                      df: 2,
                    },
                  },
                },
              },
              p: {
                docs: {},
                df: 0,
                r: {
                  docs: {
                    "scouter.AlertType": { tf: 1 },
                    "scouter.AlertZone": { tf: 1 },
                  },
                  df: 2,
                },
                o: {
                  docs: {},
                  df: 0,
                  s: {
                    docs: {},
                    df: 0,
                    i: {
                      docs: {},
                      df: 0,
                      t: {
                        docs: {},
                        df: 0,
                        o: {
                          docs: {},
                          df: 0,
                          r: {
                            docs: {},
                            df: 0,
                            y: {
                              docs: {
                                "scouter.DriftConfig": {
                                  tf: 1.4142135623730951,
                                },
                              },
                              df: 1,
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            u: {
              docs: {},
              df: 0,
              l: {
                docs: {},
                df: 0,
                e: {
                  docs: {
                    "scouter.Drifter.generate_alerts": {
                      tf: 1.4142135623730951,
                    },
                    "scouter.DriftConfig": { tf: 1.4142135623730951 },
                  },
                  df: 2,
                },
              },
            },
          },
          h: {
            docs: {},
            df: 0,
            i: {
              docs: {},
              df: 0,
              s: {
                docs: {},
                df: 0,
                t: {
                  docs: {},
                  df: 0,
                  o: {
                    docs: {},
                    df: 0,
                    g: {
                      docs: {},
                      df: 0,
                      r: {
                        docs: {},
                        df: 0,
                        a: {
                          docs: {},
                          df: 0,
                          m: {
                            docs: {},
                            df: 0,
                            s: {
                              docs: {
                                "scouter.Profiler.create_data_profile": {
                                  tf: 1,
                                },
                              },
                              df: 1,
                            },
                          },
                        },
                      },
                    },
                  },
                },
              },
            },
            a: {
              docs: {},
              df: 0,
              n: {
                docs: {},
                df: 0,
                d: {
                  docs: {},
                  df: 0,
                  l: {
                    docs: {},
                    df: 0,
                    e: {
                      docs: {},
                      df: 0,
                      r: {
                        docs: {
                          "scouter.AlertType": { tf: 1 },
                          "scouter.AlertZone": { tf: 1 },
                        },
                        df: 2,
                      },
                    },
                  },
                },
              },
              s: {
                docs: {},
                df: 0,
                h: {
                  docs: {},
                  df: 0,
                  m: {
                    docs: {},
                    df: 0,
                    a: {
                      docs: {},
                      df: 0,
                      p: { docs: { "scouter.DriftMap": { tf: 1 } }, df: 1 },
                    },
                  },
                },
              },
            },
          },
          u: {
            docs: {},
            df: 0,
            s: {
              docs: {},
              df: 0,
              e: {
                docs: {
                  "scouter.Drifter.create_drift_profile": { tf: 1 },
                  "scouter.DriftConfig": { tf: 1 },
                },
                df: 2,
                d: { docs: { "scouter.Drifter.__init__": { tf: 1 } }, df: 1 },
              },
              i: {
                docs: {},
                df: 0,
                n: {
                  docs: {},
                  df: 0,
                  g: {
                    docs: {
                      "scouter.AlertType": { tf: 1 },
                      "scouter.AlertZone": { tf: 1 },
                    },
                    df: 2,
                  },
                },
              },
            },
            c: {
              docs: {},
              df: 0,
              l: { docs: { "scouter.FeatureDriftProfile": { tf: 1 } }, df: 1 },
            },
            p: {
              docs: {},
              df: 0,
              p: {
                docs: {},
                df: 0,
                e: {
                  docs: {},
                  df: 0,
                  r: {
                    docs: { "scouter.FeatureDriftProfile": { tf: 1 } },
                    df: 1,
                  },
                },
              },
            },
          },
        },
      },
    },
    pipeline: ["trimmer"],
    _isPrebuiltIndex: true,
  };

  // mirrored in build-search-index.js (part 1)
  // Also split on html tags. this is a cheap heuristic, but good enough.
  elasticlunr.tokenizer.setSeperator(/[\s\-.;&_'"=,()]+|<[^>]*>/);

  let searchIndex;
  if (docs._isPrebuiltIndex) {
    console.info("using precompiled search index");
    searchIndex = elasticlunr.Index.load(docs);
  } else {
    console.time("building search index");
    // mirrored in build-search-index.js (part 2)
    searchIndex = elasticlunr(function () {
      this.pipeline.remove(elasticlunr.stemmer);
      this.pipeline.remove(elasticlunr.stopWordFilter);
      this.addField("qualname");
      this.addField("fullname");
      this.addField("annotation");
      this.addField("default_value");
      this.addField("signature");
      this.addField("bases");
      this.addField("doc");
      this.setRef("fullname");
    });
    for (let doc of docs) {
      searchIndex.addDoc(doc);
    }
    console.timeEnd("building search index");
  }

  return (term) =>
    searchIndex.search(term, {
      fields: {
        qualname: { boost: 4 },
        fullname: { boost: 2 },
        annotation: { boost: 2 },
        default_value: { boost: 2 },
        signature: { boost: 2 },
        bases: { boost: 2 },
        doc: { boost: 1 },
      },
      expand: true,
    });
})();
