/*
  <omega-button>: a link or a button drawn as the bottom bar's button pill, both its ends
  rounded: the frame colour round it, the band along its top, the word on the dark inside. The
  link or button is the page's own, put inside it, so it keeps its text, its focus and what it
  does; the pill is drawn behind it and it covers the whole pill.

    <omega-button><a href="https://github.com/MatanItzhaki12/omega-zero-matan">SOURCE</a></omega-button>
    <omega-button primary><a href="…/releases/latest">DOWNLOAD</a></omega-button>
    <omega-button><button type="button">COPY</button></omega-button>

  primary  the page's main action: the inside filled in the bright frame colour, the word dark.
  lit      drawn as under the cursor: the inside in the tab colour (cream for a primary one), as a
           lit row's band. It is drawn so on hover and while the link has focus too.

  Keyboard focus draws a ring round the pill, a pixel clear of it, in cream. 15 art pixels tall,
  the bottom bar's height, and as wide as its word: 6 dark pixels before the word's ink and 6
  after it, counting the letters' own blank last pixel. The word is Pixel Code one face pixel to an
  art pixel, its capitals on rows 5 to 11, two dark rows above and below them, as the bar's pill
  has its hint; give it in capitals. Areas, each a div that ::part() reaches: pill, ring.

  How the link or button inside is drawn is the component's, marked important so the page's own
  rules for links and buttons (a colour, an underline, a font) cannot reach it.

  Needs everything in foundation/ on the page (see README.md); its colours follow palette.js.
*/
(() => {
  // The pill, a 9-slice: its rounded left end, the column that stretches, its right end; on top
  // the frame line, the band and the frame line again, the inside (i), and at the bottom a row
  // of the inside that rounds off and the frame line. <action-left>'s pill, with its square
  // edge rounded off like its other end.
  const PILL = [
    '...AAA...',
    '.AAbbbAA.',
    'AAAAAAAAA',
    'AiiiiiiiA',
    '.AAiiiAA.',
    '...AAA...',
  ];
  // How the 9-slice is cut: rows above and below the row that stretches, columns either side.
  const [TOP, SIDE, BOTTOM] = [3, 4, 2];
  const H = 15;
  // The dark pixels between the frame line and the word's ink, on either side.
  const ROOM = 6;

  // The inside's colour key: unlit and lit, plain and primary.
  const INSIDE = { plain: 'k', lit: 't', primary: 'H', primaryLit: 'c' };

  // The focus ring: every pixel two pixels from the pill, on the pill's 9-slice grown by two each
  // way, the pixel next to the pill left clear. Worked out from the pill drawn at its smallest:
  // its rows with the stretching one repeated to the full height.
  function ring() {
    const full = [...PILL.slice(0, TOP), ...Array(H - TOP - BOTTOM).fill(PILL[TOP]), ...PILL.slice(-BOTTOM)];
    const w = full[0].length + 4;
    const h = H + 4;
    const pill = (x, y) => full[y - 2]?.[x - 2] !== undefined && full[y - 2][x - 2] !== '.';
    // how far (x, y) is from the pill: 1 for a pixel touching it, 2 for one beyond those, the
    // corners of the 5x5 square left out so the ring comes out round
    const near = (x, y, r) => {
      for (let dy = -r; dy <= r; dy++) {
        for (let dx = -r; dx <= r; dx++) {
          if (r === 2 && Math.abs(dx) === 2 && Math.abs(dy) === 2) continue;
          if (pill(x + dx, y + dy)) return true;
        }
      }
      return false;
    };
    const rows = Array.from({ length: h }, (_, y) =>
      Array.from({ length: w }, (_, x) => (near(x, y, 2) && !near(x, y, 1) ? 'c' : '.')).join(''));
    // the 9-slice: the ends and the middle column, the rows above and below the middle one
    const mid = SIDE + 2;
    const cut = rows.map(row => row.slice(0, mid) + row[mid] + row.slice(-mid));
    return [...cut.slice(0, TOP + 2), cut[TOP + 2], ...cut.slice(-(BOTTOM + 2))];
  }
  const RING = ring();

  const template = document.createElement('template');
  template.innerHTML = `
    <style>
      /* text-rendering: Pixel Code drawn without the browser's hinting, which smears its edges by
         part of a pixel even where it sits on whole pixels */
      :host {
        display: inline-flex;
        position: relative;
        vertical-align: top;
        height: calc(${H} * var(--u));
        text-rendering: geometricPrecision;
      }
      :host([hidden]) { display: none; }

      .pill, .ring {
        position: absolute;
        pointer-events: none;
        border-style: solid;
        border-color: transparent;
        image-rendering: pixelated;
      }
      .pill {
        inset: 0;
        border-width: calc(${TOP} * var(--u)) calc(${SIDE} * var(--u)) calc(${BOTTOM} * var(--u));
        border-image-source: var(--pill);
        border-image-slice: ${TOP} ${SIDE} ${BOTTOM} fill;
        border-image-repeat: stretch;
      }
      :host(:hover) .pill, :host(:focus-within) .pill, :host([lit]) .pill { border-image-source: var(--pill-lit); }
      .ring {
        display: none;
        inset: calc(-2 * var(--u));
        border-width: calc(${TOP + 2} * var(--u)) calc(${SIDE + 2} * var(--u)) calc(${BOTTOM + 2} * var(--u));
        border-image-source: var(--ring);
        border-image-slice: ${TOP + 2} ${SIDE + 2} ${BOTTOM + 2};
        border-image-repeat: stretch;
      }
      .ring.on { display: block; }

      /* The link or button covers the whole pill, its word on the pill's line 3 so the capitals
         are on rows 5 to 11, ${ROOM} dark pixels from either frame line to its ink. Important, so
         the page's rules for links and buttons stay off it. */
      ::slotted(a), ::slotted(button) {
        all: unset !important;
        position: relative !important;
        display: block !important;
        box-sizing: border-box !important;
        height: 100% !important;
        padding: calc(${TOP} * var(--u)) calc(${ROOM} * var(--u)) 0 calc(${ROOM + 1} * var(--u)) !important;
        font: 400 calc(9 * var(--u)) / calc(10 * var(--u)) var(--oz-font) !important;
        white-space: nowrap !important;
        color: var(--oz-cream) !important;
        cursor: pointer !important;
        outline: none !important;
      }
      :host([primary]) ::slotted(a), :host([primary]) ::slotted(button) { color: var(--oz-dark) !important; }
    </style>
    <div class="pill" part="pill"></div>
    <div class="ring" part="ring"></div>
    <slot></slot>`;

  customElements.define('omega-button', class extends OmegaPixel.Element {
    static observedAttributes = ['primary'];

    constructor() {
      super();
      this.attachShadow({ mode: 'open' }).append(template.content.cloneNode(true));
      // the ring only for focus the keyboard gave, as :focus-visible has it
      this.addEventListener('focusin', event => this.ring(event.target.matches(':focus-visible')));
      this.addEventListener('focusout', () => this.ring(false));
    }

    // One of its areas, by class.
    area(name) {
      return this.shadowRoot.querySelector(`.${name}`);
    }

    paint() {
      const { sprite } = OmegaPixel;
      const primary = this.hasAttribute('primary');
      const inside = key => PILL.map(row => row.replaceAll('i', key));
      const pill = this.area('pill').style;
      pill.setProperty('--pill', sprite(inside(primary ? INSIDE.primary : INSIDE.plain), this));
      pill.setProperty('--pill-lit', sprite(inside(primary ? INSIDE.primaryLit : INSIDE.lit), this));
      this.area('ring').style.setProperty('--ring', sprite(RING, this));
    }

    ring(on) {
      this.area('ring').classList.toggle('on', on);
    }

    attributeChangedCallback() {
      if (this.isConnected) this.paint();
    }
  });
})();
