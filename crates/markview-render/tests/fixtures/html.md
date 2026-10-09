# HTML compatibility

<a id="target"></a>Anchor before text; [jump](#target) after it. <!-- hidden comment -->Visible text.

<p><b>Bold</b> <i>italic</i> <code>code</code> and <sup>2</sup>.</p>

<div>

## Heading inside a group

- Markdown inside a div
- Second item

<section>

> Nested grouped quote.

</section>
</div>

<article>

Article **content**.

</article>
<aside>

Aside *content*.

</aside>
<figure>

![Grouped image](tiles.png)

<figcaption>

Figure caption text.

</figcaption>
</figure>
<header>

Header text.

</header>
<main>

Main text.

</main>
<nav>

Navigation [link](#target).

</nav>
<footer>

Footer text.

</footer>
<address>

Address text.

</address>

Unsupported inline <span>markup</span> keeps its source.

<pre>literal *stars* and <b>tags</b></pre>

<script>const literal = "<b>source</b>";</script>
