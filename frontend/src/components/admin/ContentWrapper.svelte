<script>
    import {fade} from "svelte/transition";

    export let eventsCollapsed = true;
    export let eventsWide = false;

    const tDur = 100;
</script>

<div
        class="content"
        class:mr={!eventsCollapsed && !eventsWide}
        class:mrCollapsed={eventsCollapsed && !eventsWide}
        class:mrWide={!eventsCollapsed && eventsWide}
        in:fade={{delay: tDur * 2, duration: tDur}}
        out:fade={{duration: tDur}}
>
    <slot></slot>
</div>

<style>
    .content {
        display: flex;
        flex-direction: column;
        justify-content: center;
        align-items: center;
        height: 100vh;
        flex: 1;
        overflow-y: auto;
        transition: all 250ms ease-in-out;
    }

    .mr {
        margin-right: var(--width-events);
    }

    .mrCollapsed {
        margin-right: var(--width-events-collapsed);
    }

    .mrWide {
        margin-right: var(--width-events-wide);
    }
</style>
