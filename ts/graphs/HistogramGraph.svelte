<script lang="typescript">
    import type { I18n } from "anki/i18n";

    import AxisTicks from "./AxisTicks.svelte";
    import NoDataOverlay from "./NoDataOverlay.svelte";
    import CumulativeOverlay from "./CumulativeOverlay.svelte";
    import HoverColumns from "./HoverColumns.svelte";

    import type { HistogramData } from "./histogram-graph";
    import { histogramGraph } from "./histogram-graph";
    import { defaultGraphBounds } from "./graph-helpers";

    export let data: HistogramData | null = null;
    export let i18n: I18n;

    let bounds = defaultGraphBounds();
    let svg = null as HTMLElement | SVGElement | null;

    $: histogramGraph(svg as SVGElement, bounds, data);
</script>

<svg bind:this={svg} viewBox={`0 0 ${bounds.width} ${bounds.height}`}>
    <g class="bars" />
    <HoverColumns />
    <CumulativeOverlay />
    <AxisTicks {bounds} />
    <NoDataOverlay {bounds} {i18n} />
</svg>
