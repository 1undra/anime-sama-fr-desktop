(() => {
    const globalAdSelector = "#bannLeft,#bannRight,#bannBot,#squBot,.ad-container,.ad-label,.ad-content,.ad-unit,.ad-zone,.ad-banner,.adbox,.pub-btag,.pub-ordi,.pub-tel,.afs_ads,.ad-placement,[data-adcash-zone],[data-onclicka-zone],[data-pub-suivi],#vidmolyadblocktest,.jw-ads,.jwp-ad,ins.adsbygoogle,iframe[src*='/pub/']";
    const installGlobalAdStyle = () => {
        if (!document.head || document.documentElement == null) {
            window.setTimeout(installGlobalAdStyle, 20);
            return;
        }
        if (window.__ANIME_SAMA_ADBLOCK__ === false) return;
        if (!document.getElementById("anime-sama-global-ad-style")) {
            const style = document.createElement("style");
            style.id = "anime-sama-global-ad-style";
            style.textContent = globalAdSelector + "{display:none!important;visibility:hidden!important;width:0!important;height:0!important;overflow:hidden!important}";
            document.head.appendChild(style);
        }
    };
    installGlobalAdStyle();
    const watchGlobalAds = () => {
        if (document.documentElement == null) {
            window.setTimeout(watchGlobalAds, 50);
            return;
        }
        new MutationObserver(installGlobalAdStyle).observe(document.documentElement, { childList: true, subtree: true });
    };
    watchGlobalAds();

    const adDomainSet = new Set();
    for (const domain of window.__ANIME_SAMA_AD_DOMAINS__ || []) adDomainSet.add(String(domain).toLowerCase());
    const safeHostPattern = /(^|\.)(anime-sama\.to|ansembed\.net|vmpx\.online|upvm\.online|jwpcdn\.com|jwplayer\.com|staticmoly\.me|flagcdn\.com|gstatic\.com|cloudflare\.com|cloudflare\.net|jsdelivr\.net|jquery\.com|fontawesome\.com|githubusercontent\.com)(\.|$)/i;
    const isAdUrl = value => {
        if (window.__ANIME_SAMA_ADBLOCK__ === false) return false;
        let url;
        try {
            url = new URL(String(value), location.href);
        } catch (_) {
            return false;
        }
        if (!/^https?:$/.test(url.protocol)) return false;
        const host = url.hostname.toLowerCase().replace(/\.$/, "");
        if (safeHostPattern.test(host)) return false;
        if (adDomainSet.has(host)) return true;
        const labels = host.split(".");
        for (let i = 1; i < labels.length; i++) {
            if (adDomainSet.has(labels.slice(i).join("."))) return true;
        }
        if (/^https?:$/.test(url.protocol)) {
            const path = url.pathname.toLowerCase();
            if (path.includes("/ad/") || path.includes("/pub/") || path.includes("visit.php")) return true;
        }
        return (url.searchParams.has("zoneid") || url.searchParams.has("request_ab2"))
            && (url.searchParams.has("rb") || url.searchParams.has("zoneid"));
    };
    const blockedRequests = new WeakSet();
    const xhrOpen = XMLHttpRequest.prototype.open;
    const xhrSend = XMLHttpRequest.prototype.send;
    XMLHttpRequest.prototype.open = function (method, url, ...rest) {
        if (isAdUrl(url)) {
            blockedRequests.add(this);
            return;
        }
        blockedRequests.delete(this);
        return xhrOpen.call(this, method, url, ...rest);
    };
    XMLHttpRequest.prototype.send = function (...args) {
        if (blockedRequests.has(this)) return;
        return xhrSend.apply(this, args);
    };
    const nativeFetch = window.fetch;
    window.fetch = function (input, ...rest) {
        if (isAdUrl(typeof input == "string" ? input : input && input.url)) return Promise.reject(new Error("blocked"));
        return nativeFetch.call(this, input, ...rest);
    };
    const nativeOpen = window.open;
    window.open = function (url, ...rest) {
        if (isAdUrl(url)) return null;
        return nativeOpen.call(this, url, ...rest);
    };
    const nativeBeacon = navigator.sendBeacon && navigator.sendBeacon.bind(navigator);
    if (nativeBeacon) {
        navigator.sendBeacon = function (url, ...rest) {
            if (isAdUrl(url)) return true;
            return nativeBeacon(url, ...rest);
        };
    }
    const removeAdNodes = () => {
        for (const element of document.querySelectorAll("iframe[src],script[src],img[src],a[href]")) {
            const target = element.src || element.href;
            if (target && isAdUrl(target)) element.remove();
        }
    };
    const watchAdNodes = () => {
        if (document.documentElement == null) {
            window.setTimeout(watchAdNodes, 50);
            return;
        }
        let scheduled = false;
        new MutationObserver(() => {
            if (scheduled) return;
            scheduled = true;
            window.setTimeout(() => {
                scheduled = false;
                removeAdNodes();
            }, 50);
        }).observe(document.documentElement, { childList: true, subtree: true });
        removeAdNodes();
    };
    watchAdNodes();

    document.addEventListener("click", event => {
        const anchor = event.target && event.target.closest && event.target.closest("a[href]");
        if (anchor && isAdUrl(anchor.href)) {
            event.preventDefault();
            event.stopPropagation();
            anchor.remove();
        }
    }, true);

    if (/ansembed|vidmoly|jwplayer|vmpx|upvm/.test(location.hostname)) {
        const clickResume = () => {
            const button = document.getElementById("resume-yes") || document.querySelector(".resume-btn-new");
            if (!button || button.dataset.animeSamaResume) return;
            button.dataset.animeSamaResume = "1";
            window.setTimeout(() => button.click(), 200);
        };
        const watchResume = () => {
            if (document.documentElement == null) {
                window.setTimeout(watchResume, 50);
                return;
            }
            new MutationObserver(clickResume).observe(document.documentElement, { childList: true, subtree: true });
            clickResume();
            window.setInterval(clickResume, 500);
        };
        watchResume();
    }

    const playerFrame = window !== window.top || /uqload|voe|vidmoly|vmp4|senvid|dood|streamtape|mp4upload|mixdrop|filemoon|jwplayer|ansembed|vmpx|upvm/.test(location.hostname);
    if (window.__ANIME_SAMA_ADBLOCK__ !== false) {
        window.open = () => null;
        const frameHost = location.hostname;
        document.addEventListener("click", event => {
            const anchor = event.target && event.target.closest && event.target.closest("a[href]");
            if (!anchor) return;
            try {
                const url = new URL(anchor.href, location.href);
                if (url.hostname !== frameHost && !safeHostPattern.test(url.hostname)) {
                    event.preventDefault();
                    event.stopPropagation();
                    anchor.remove();
                }
            } catch (_) {}
        }, true);
        const nativeSubmit = HTMLFormElement.prototype.submit;
        HTMLFormElement.prototype.submit = function () {
            try {
                const url = new URL(this.action || location.href, location.href);
                if (url.hostname !== frameHost && !safeHostPattern.test(url.hostname)) return;
            } catch (_) {}
            return nativeSubmit.call(this);
        };
        if (playerFrame) {
            const playerUrl = location.href;
            window.setInterval(() => {
                if (location.href === playerUrl) return;
                try {
                    const url = new URL(location.href);
                    if (url.hostname !== frameHost && !safeHostPattern.test(url.hostname)) {
                        location.replace(playerUrl);
                    }
                } catch (_) {}
            }, 300);
        }
    }

    if (playerFrame) {
        const skipAdBreak = () => {
            const video = document.querySelector("video");
            if (!video) {
                window.setTimeout(skipAdBreak, 500);
                return;
            }
            let lastPlay = 0;
            let skipped = 0;
            video.addEventListener("play", () => { lastPlay = Date.now(); });
            video.addEventListener("pause", () => {
                if (video.currentTime < 0.5 && Date.now() - lastPlay < 1200 && skipped < 3) {
                    skipped++;
                    window.setTimeout(() => video.play().catch(() => {}), 150);
                }
            });
        };
        window.setTimeout(skipAdBreak, 1000);
    }

    const start = tauri => {
    document.addEventListener("contextmenu", event => event.preventDefault(), true);
    window.addEventListener("keydown", event => {
        if (event.key == "F12" || (event.ctrlKey && event.shiftKey && ["I", "J", "C"].includes(event.key.toUpperCase()))) event.preventDefault();
    }, true);

    const core = tauri.core || tauri;
    const invoke = (command, args) => core.invoke(command, args).catch(() => undefined);
    const listen = (name, handler) => tauri.event.listen(name, handler).catch(() => undefined);

    const adSelector = "#bannLeft,#bannRight,#bannBot,#squBot,.ad-container,.ad-label,.ad-content,.pub-btag,.pub-ordi,.pub-tel,[data-adcash-zone],[data-onclicka-zone],[data-pub-suivi],iframe[src*='/pub/']";
    const adStyle = adSelector + "{display:none!important;visibility:hidden!important;width:0!important;height:0!important;overflow:hidden!important}";
    let adblockEnabled = window.__ANIME_SAMA_ADBLOCK__ !== false;
    let adblockObserver = null;
    const applyAdBlockStyle = () => {
        const style = document.getElementById("anime-sama-adblock-style");
        if (adblockEnabled) {
            if (!style) {
                const element = document.createElement("style");
                element.id = "anime-sama-adblock-style";
                element.textContent = adStyle;
                document.head.appendChild(element);
            }
            document.querySelectorAll(adSelector).forEach(element => element.setAttribute("data-anime-sama-ad-hidden", ""));
        } else {
            if (style) style.remove();
            document.querySelectorAll("[data-anime-sama-ad-hidden]").forEach(element => element.removeAttribute("data-anime-sama-ad-hidden"));
        }
    };
    const startAdBlock = () => {
        if (!document.head) {
            window.setTimeout(startAdBlock, 20);
            return;
        }
        applyAdBlockStyle();
        if (!adblockObserver) {
            adblockObserver = new MutationObserver(applyAdBlockStyle);
            adblockObserver.observe(document.documentElement, {
                childList: true,
                subtree: true,
                attributes: true,
                attributeFilter: ["class", "style", "src"]
            });
            setInterval(applyAdBlockStyle, 1000);
        }
    };
    startAdBlock();
    listen("adblock-changed", event => {
        adblockEnabled = event.payload === true;
        if (document.head) applyAdBlockStyle();
    });

    const is = {
        string(value) {
            return typeof value == "string" ? value.length > 0 : false;
        },
        number(value) {
            return isNaN(Number(value)) == false;
        },
        array(value) {
            return Array.isArray(value) == true ? value.length > 0 : false;
        },
        url(value) {
            try {
                new URL(value);
                return true;
            } catch (error) {
                return false;
            }
        }
    };

    const storage = {
        has(name) {
            if (!is.string(name)) return false;
            for (let index = 0; index < window.localStorage.length; index++) {
                if (window.localStorage.key(index) == name) return true;
            }
            return false;
        },
        get(name) {
            try {
                if (this.has(name)) return JSON.parse(window.localStorage.getItem(name));
                return undefined;
            } catch (error) {
                return undefined;
            }
        },
        set(name, value) {
            if (!is.string(name)) return false;
            if (typeof value != "string") value = JSON.stringify(value);
            window.localStorage.setItem(name, value);
            return true;
        }
    };

    const windowControlIcons = {
        minimize: `<svg viewBox="0 0 10 10"><path d="M0 5h10"/></svg>`,
        maximize: `<svg viewBox="0 0 10 10"><rect x="0.5" y="0.5" width="9" height="9"/></svg>`,
        restore: `<svg viewBox="0 0 10 10"><path d="M2.5 2.5v-2h7v7h-2"/><rect x="0.5" y="2.5" width="7" height="7"/></svg>`,
        close: `<svg viewBox="0 0 10 10"><path d="M0 0l10 10M10 0L0 10"/></svg>`
    };

    const windowControlStyle = `#desktop-window-controls {display: flex;align-self: stretch;margin-left: auto;-webkit-app-region: no-drag;}
#desktop-window-controls button {display: flex;align-items: center;justify-content: center;width: 46px;padding: 0;border: 0;background: transparent;color: #CBD5E1;cursor: default;transition: background-color 150ms, color 150ms;}
#desktop-window-controls button:hover {background: rgba(255, 255, 255, 0.1);color: #FFF;}
#desktop-window-controls button:active {background: rgba(255, 255, 255, 0.05);}
#desktop-window-controls button[data-control="close"]:hover {background: #E81123;color: #FFF;}
#desktop-window-controls svg {width: 10px;height: 10px;fill: none;stroke: currentColor;stroke-width: 1;}
body > #desktop-window-controls {position: fixed;top: 0;right: 0;height: 48px;z-index: 2147483647}html.anime-sama-media-playing #desktop-window-controls{display:none!important}`;

    const setWindowControls = () => {
        if (window !== window.top) return false;
        if (document.getElementById("desktop-window-controls")) return true;
        if (!document.head || !document.body) return false;
        const style = document.createElement("style");
        style.textContent = windowControlStyle;
        document.head.appendChild(style);

        const controls = document.createElement("div");
        controls.id = "desktop-window-controls";
        const commands = {
            minimize: "window_minimize",
            maximize: "window_maximize",
            close: "window_close"
        };
        for (const control of ["minimize", "maximize", "close"]) {
            const button = document.createElement("button");
            button.type = "button";
            button.dataset.control = control;
            button.innerHTML = windowControlIcons[control];
            button.addEventListener("click", () => invoke(commands[control]));
            controls.appendChild(button);
        }
        (document.querySelector("nav.asn-navbar") || document.body).appendChild(controls);
        const nav = document.querySelector("nav.asn-navbar");
        if (nav) nav.setAttribute("data-tauri-drag-region", "");

        const maximize = controls.querySelector(`[data-control="maximize"]`);
        const drawMaximize = maximized => {
            maximize.innerHTML = maximized ? windowControlIcons.restore : windowControlIcons.maximize;
        };
        const appWindow = tauri.window.getCurrentWindow();
        appWindow.isMaximized().then(drawMaximize).catch(() => undefined);
        appWindow.onResized(() => {
            appWindow.isMaximized().then(drawMaximize).catch(() => undefined);
        });
        return true;
    };

    const startWindowControls = () => {
        if (window !== window.top) return;
        if (setWindowControls()) return;
        const timer = setInterval(() => {
            if (setWindowControls()) clearInterval(timer);
        }, 250);
    };
    if (document.readyState == "loading") window.addEventListener("DOMContentLoaded", startWindowControls);
    else startWindowControls();

    const setMediaControls = hidden => {
        const root = document.documentElement;
        if (root) root.classList.toggle("anime-sama-media-playing", hidden === true);
    };
    const updateMediaControls = () => {
        setMediaControls(!!document.fullscreenElement);
    };
    document.addEventListener("fullscreenchange", updateMediaControls);
    window.addEventListener("DOMContentLoaded", updateMediaControls);
    setInterval(updateMediaControls, 1000);

    const attachMedia = video => {
        if (!video || video.__animeSamaMedia) return;
        video.__animeSamaMedia = true;
        video.addEventListener("play", () => invoke("media_state", { state: "playing" }));
        video.addEventListener("pause", () => invoke("media_state", { state: "paused" }));
        video.addEventListener("ended", () => invoke("media_state", { state: "paused" }));
    };

    const scanMedia = () => {
        document.querySelectorAll("video").forEach(attachMedia);
        document.querySelectorAll("iframe").forEach(frame => {
            try {
                frame.contentDocument?.querySelectorAll("video").forEach(attachMedia);
            } catch (error) {
                return;
            }
        });
    };

    const startMediaBridge = () => {
        scanMedia();
        new MutationObserver(scanMedia).observe(document.documentElement, {
            childList: true,
            subtree: true
        });
        setInterval(scanMedia, 2000);
        setInterval(() => {
            const video = Array.from(document.querySelectorAll("video")).find(item => isNaN(item.duration) == false && item.duration > 0 && item.paused == false);
            if (!video) return;
            invoke("video_progress", {
                current: video.currentTime,
                duration: video.duration
            });
        }, 15000);
    };

    window.addEventListener("DOMContentLoaded", () => {
        setWindowControls();

        if (window.location.pathname.endsWith("error.html")) {
            const redirect = document.getElementById("redirect");
            if (redirect) redirect.addEventListener("click", event => {
                event.preventDefault();
                invoke("window_home");
            });
            return;
        }

        startMediaBridge();

        const currentPathName = window.location.pathname.endsWith("/") ? window.location.pathname.slice(0, window.location.pathname.lastIndexOf("/")) : window.location.pathname;

        if (currentPathName.length <= 0 || currentPathName == "/profil") {
            document.querySelectorAll(".supprHistorique").forEach(element => element.addEventListener("click", event => invoke("delete_history", { url: event.currentTarget.nextSibling.href })));
        }

        if (currentPathName == "/profil") {
            const supprAllHistorique = document.getElementById("supprAllHistorique");
            const supprAllWatchlist = document.getElementById("supprAllWatchlist");
            const supprAllFavoris = document.getElementById("supprAllFavoris");
            if (supprAllHistorique) supprAllHistorique.addEventListener("click", () => invoke("delete_all_history"));
            if (supprAllWatchlist) supprAllWatchlist.addEventListener("click", () => invoke("delete_all_watchlist"));
            if (supprAllFavoris) supprAllFavoris.addEventListener("click", () => invoke("delete_all_favorite"));
            document.querySelectorAll(".supprWatchlist").forEach(element => element.addEventListener("click", event => invoke("delete_watchlist", { url: event.currentTarget.nextSibling.href })));
            document.querySelectorAll(".supprFavoris").forEach(element => element.addEventListener("click", event => invoke("delete_favorite", { url: event.currentTarget.nextSibling.href })));
        } else if (currentPathName.startsWith("/catalogue/")) {
            const episode = document.getElementById("savedEpisodeId");
            const scan = document.getElementById("savedChapitreId");
            const watchlist = document.getElementById("addWatchlist");
            const favorite = document.getElementById("addFavoris");

            const getCurrentEpisode = () => {
                const current = {};
                const select = document.getElementById("selectEpisodes");
                const option = select && select.options ? select.options[select.selectedIndex] : undefined;
                const season = document.getElementById("avOeuvre");
                if (option && is.string(option.text)) {
                    current.name = option.text;
                    current.index = select.selectedIndex;
                }
                if (season && is.string(season.textContent)) current.season = season.textContent;
                return current;
            };

            const getHistoData = () => {
                const data = {};
                let index = storage.get("histoUrl");
                index = is.array(index) ? index.findIndex(pathname => is.string(pathname) && (pathname.endsWith("/") ? pathname.slice(0, pathname.lastIndexOf("/")) : pathname) == currentPathName) : -1;
                if (index < 0) return data;

                let type = storage.get("histoType");
                type = is.array(type) ? type[index] : undefined;
                if (!is.string(type)) return data;

                const title = storage.get("histoNom");
                const image = storage.get("histoImg");
                const language = storage.get("histoLang");
                let episode = storage.get("histoEp");
                data.type = type.toUpperCase() == "FILM" ? "MOVIE" : type.toUpperCase() == "SCANS" ? "SCAN" : type;
                data.title = is.array(title) ? title[index] : undefined;
                data.image = is.array(image) ? image[index] : undefined;
                data.language = is.array(language) ? language[index] : undefined;
                data.url = window.location.pathname;
                data.url_info = `/catalogue/${window.location.pathname.split("/catalogue/")[1].split("/")[0]}`;
                episode = is.array(episode) ? episode[index] : undefined;

                if (data.type == "SCAN") {
                    const saved = storage.get(`savedChapNb${data.url}`);
                    if (is.string(episode)) data.chapter = episode.split("Chapitre")[1].trim();
                    else if (is.number(episode)) data.chapter = Number(episode).toString();
                    if (is.number(saved)) data.saved = saved;
                } else {
                    let saved = storage.get(`savedEpNb${data.url}`);
                    const current = getCurrentEpisode();
                    if (is.string(current.name)) episode = current.name;
                    if (is.number(current.index)) saved = current.index;
                    if (is.string(current.season)) type = current.season;
                    if (is.string(episode)) data.episode = episode.split("Episode")[1].trim();
                    else if (is.number(episode)) data.episode = Number(episode).toString();
                    data.season = type.trim().replace("partie", "-");
                    if (is.number(saved)) data.saved = saved;
                }
                return data;
            };

            const getWatchData = () => {
                const data = {};
                let index = storage.get("watchlistUrl");
                index = is.array(index) ? index.findIndex(pathname => is.string(pathname) && (pathname.endsWith("/") ? pathname.slice(0, pathname.lastIndexOf("/")) : pathname) == currentPathName) : -1;
                if (index < 0) return data;
                const title = storage.get("watchlistNom");
                const image = storage.get("watchlistImg");
                data.url = window.location.pathname;
                data.title = is.array(title) ? title[index] : undefined;
                data.image = is.array(image) ? image[index] : undefined;
                return data;
            };

            const getFavoData = () => {
                const data = {};
                let index = storage.get("favoriUrl");
                index = is.array(index) ? index.findIndex(pathname => is.string(pathname) && (pathname.endsWith("/") ? pathname.slice(0, pathname.lastIndexOf("/")) : pathname) == currentPathName) : -1;
                if (index < 0) return data;
                const title = storage.get("favoriNom");
                const image = storage.get("favoriImg");
                data.url = window.location.pathname;
                data.title = is.array(title) ? title[index] : undefined;
                data.image = is.array(image) ? image[index] : undefined;
                return data;
            };

            const selectEpisodes = document.getElementById("selectEpisodes");
            if (episode || selectEpisodes) {
                let lastEpisodeSent;
                const sendEpisodeChange = () => {
                    const data = getHistoData();
                    if (!is.string(data.episode)) return;
                    const current = `${data.url}|${data.season}|${data.episode}`;
                    if (current == lastEpisodeSent) return;
                    lastEpisodeSent = current;
                    invoke("episode_change", { data });
                };
                if (episode) new MutationObserver(() => sendEpisodeChange()).observe(episode, { childList: true });
                if (selectEpisodes) {
                    selectEpisodes.addEventListener("change", () => setTimeout(sendEpisodeChange, 50));
                    new MutationObserver(() => setTimeout(sendEpisodeChange, 50)).observe(selectEpisodes, { childList: true });
                    setInterval(sendEpisodeChange, 1000);
                }
            }

            if (scan) {
                new MutationObserver(() => invoke("scan_change", { data: getHistoData() })).observe(scan, { childList: true });
                invoke("scan_change", { data: getHistoData() });
            }

            if (watchlist) watchlist.addEventListener("click", () => {
                const checkedWatchlist = document.getElementById("checkedWatchlist");
                if (checkedWatchlist) new MutationObserver((record, observer) => {
                    invoke("watchlist_change", { data: getWatchData() });
                    observer.disconnect();
                }).observe(checkedWatchlist, { attributes: true });
            });

            if (favorite) favorite.addEventListener("click", () => {
                const checkedFavoris = document.getElementById("checkedFavoris");
                if (checkedFavoris) new MutationObserver((record, observer) => {
                    invoke("favorite_change", { data: getFavoData() });
                    observer.disconnect();
                }).observe(checkedFavoris, { attributes: true });
            });
        }

        listen("import-history", event => importHistory(event.payload));
        listen("import-watchlist", event => importWatchlist(event.payload));
        listen("import-favorite", event => importFavorite(event.payload));
    });

    const importHistory = data => {
        if (!data || !is.url(data.url) || !is.string(data.type)) return;
        const isScan = data.type.toUpperCase() == "SCANS";
        const url = new URL(data.url).pathname;
        let histoUrl = storage.get("histoUrl");
        if (!is.array(histoUrl)) histoUrl = [];
        const alreadyExist = histoUrl.findIndex(pathname => is.string(pathname) && (pathname.endsWith("/") ? pathname.slice(0, pathname.lastIndexOf("/")) : pathname) == (url.endsWith("/") ? url.slice(0, url.lastIndexOf("/")) : url));
        let index = alreadyExist;
        if (alreadyExist < 0) {
            histoUrl.push(url);
            storage.set("histoUrl", histoUrl);
            index = histoUrl.length - 1;
            let histoType = storage.get("histoType");
            if (!is.array(histoType)) histoType = [];
            histoType[index] = data.type;
            storage.set("histoType", histoType);
            if (is.string(data.title)) {
                let histoNom = storage.get("histoNom");
                if (!is.array(histoNom)) histoNom = [];
                histoNom[index] = data.title;
                storage.set("histoNom", histoNom);
            }
            if (is.url(data.image)) {
                let histoImg = storage.get("histoImg");
                if (!is.array(histoImg)) histoImg = [];
                histoImg[index] = data.image;
                storage.set("histoImg", histoImg);
            }
            if (is.string(data.language)) {
                let histoLang = storage.get("histoLang");
                if (!is.array(histoLang)) histoLang = [];
                histoLang[index] = data.language;
                storage.set("histoLang", histoLang);
            }
        }
        if (is.string(data.episode)) {
            let histoEp = storage.get("histoEp");
            if (!is.array(histoEp)) histoEp = [];
            histoEp[index] = data.episode;
            storage.set("histoEp", histoEp);
            storage.set(isScan ? `savedChapName${url}` : `savedEpName${url}`, JSON.stringify(data.episode));
        }
        if (is.number(data.saved)) storage.set(isScan ? `savedChapNb${url}` : `savedEpNb${url}`, data.saved);
    };

    const importWatchlist = data => {
        if (!data || !is.url(data.url)) return;
        const url = new URL(data.url).pathname;
        let watchlistUrl = storage.get("watchlistUrl");
        if (!is.array(watchlistUrl)) watchlistUrl = [];
        const alreadyExist = watchlistUrl.findIndex(pathname => is.string(pathname) && (pathname.endsWith("/") ? pathname.slice(0, pathname.lastIndexOf("/")) : pathname) == (url.endsWith("/") ? url.slice(0, url.lastIndexOf("/")) : url));
        if (alreadyExist >= 0) return;
        watchlistUrl.push(url);
        storage.set("watchlistUrl", watchlistUrl);
        const index = watchlistUrl.length - 1;
        if (is.string(data.title)) {
            let watchlistNom = storage.get("watchlistNom");
            if (!is.array(watchlistNom)) watchlistNom = [];
            watchlistNom[index] = data.title;
            storage.set("watchlistNom", watchlistNom);
        }
        if (is.url(data.image)) {
            let watchlistImg = storage.get("watchlistImg");
            if (!is.array(watchlistImg)) watchlistImg = [];
            watchlistImg[index] = data.image;
            storage.set("watchlistImg", watchlistImg);
        }
    };

    const importFavorite = data => {
        if (!data || !is.url(data.url)) return;
        const url = new URL(data.url).pathname;
        let favoriUrl = storage.get("favoriUrl");
        if (!is.array(favoriUrl)) favoriUrl = [];
        const alreadyExist = favoriUrl.findIndex(pathname => is.string(pathname) && (pathname.endsWith("/") ? pathname.slice(0, pathname.lastIndexOf("/")) : pathname) == (url.endsWith("/") ? url.slice(0, url.lastIndexOf("/")) : url));
        if (alreadyExist >= 0) return;
        favoriUrl.push(url);
        storage.set("favoriUrl", favoriUrl);
        const index = favoriUrl.length - 1;
        if (is.string(data.title)) {
            let favoriNom = storage.get("favoriNom");
            if (!is.array(favoriNom)) favoriNom = [];
            favoriNom[index] = data.title;
            storage.set("favoriNom", favoriNom);
        }
        if (is.url(data.image)) {
            let favoriImg = storage.get("favoriImg");
            if (!is.array(favoriImg)) favoriImg = [];
            favoriImg[index] = data.image;
            storage.set("favoriImg", favoriImg);
        }
    };
    };

    const waitForTauri = () => {
        const tauri = window.__TAURI__;
        if (!tauri) {
            window.setTimeout(waitForTauri, 20);
            return;
        }
        start(tauri);
    };
    waitForTauri();
})();
