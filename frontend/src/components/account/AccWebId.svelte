<script>
    import Button from "$lib/Button.svelte";
    import {fade, slide} from 'svelte/transition';
    import {putUserWebIdData} from "../../utils/dataFetching.js";
    import Switch from "$lib/Switch.svelte";
    import {buidlWebIdUri} from "../../utils/helpers.js";

    export let t;
    export let webIdData;
    // export let viewModePhone;

    const btnWidth = "12rem";

    let err = '';
    let success = false;
    let successEmailConfirm = false;
    let expertMode = !!webIdData.data;
    let webIdLink = buidlWebIdUri(webIdData.user_id);

    // do not set any value here - will be bound to validate function in <AccWebIdEntries/>
    let getData;
    let validateData;

    $: if (success) {
        setTimeout(() => {
            success = false;
        }, 3000);
    }

    async function onSubmit() {
        err = '';

        let res = await putUserWebIdData(webIdData.user_id, webIdData);
        if (res.ok) {
            success = true;
        } else {
            let body = await res.json();
            err = body.message;
        }
    }

</script>

<div class="wrapper">
    <div class="container">
        <p>{t.webIdDesc}</p>

        <p>
            <a href={webIdLink} target="_blank">
                {@html webIdLink.replace('/webid/', '/webid/<wbr/>')}
            </a>
        </p>

        <div class="row">
            <div class="label">
                E-Mail
            </div>
            <Switch bind:selected={webIdData.expose_email}/>
        </div>

        <div class="row">
            <div class="label">
                {t.webIdExpertMode}
            </div>
            <Switch bind:selected={expertMode}/>
        </div>

        {#if expertMode}
            <div transition:slide>
                <p>{t.webIdDescData}</p>
<!--                <AccWebIdEntries-->
<!--                        bind:t-->
<!--                        bind:webIdData-->
<!--                        bind:viewModePhone-->
<!--                        bind:getData-->
<!--                        bind:validateData-->
<!--                />-->
                <textarea
                        class="font-mono text"
                        rows={15}
                        cols={40}
                        bind:value={webIdData.custom_triples}
                ></textarea>
            </div>
        {/if}

        <Button width={btnWidth} on:click={onSubmit} level={1}>
            {t.save.toUpperCase()}
        </Button>

        <div class="bottom">
            {#if success}
                <div class="success" transition:fade>
                    Update successful
                </div>
            {:else if err}
                <div class="err" transition:fade>
                    {err}
                </div>
            {/if}
        </div>
    </div>
</div>

<style>
    p {
        margin: 0 .5rem .5rem .5rem;
    }

    textarea {
        margin: .5rem;
        resize: none;
    }

    .bottom {
        height: 1em;
    }

    .container {
        padding: 0 5px;
        display: flex;
        flex-direction: column;
    }

    .err {
        width: 230px;
        margin: -5px 10px 0 35px;
        padding-right: 5px;
        color: var(--col-err);
    }

    .err {
        margin: 5px;
        color: var(--col-err);
    }

    .label {
        width: 12rem;
    }

    .row {
        margin: .5rem;
        display: flex;
        gap: .5rem;
    }

    .success {
        margin: 5px;
        color: var(--col-ok);
    }

    .wrapper {
        margin-top: 10px;
        display: flex;
        flex-direction: row;
    }
</style>
