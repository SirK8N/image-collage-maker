import React, { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  AlertCircle,
  Download,
  FileSpreadsheet,
  Grid,
  LayoutGrid,
  Loader2,
  Play,
  Sparkles,
  X,
} from "lucide-react";
import { LogDrawer } from "./components/LogDrawer";
import { MosaicOptionsSection } from "./components/MosaicOptionsSection";
import { PreviewCanvas } from "./components/PreviewCanvas";
import { SortOptionsSection } from "./components/SortOptionsSection";
import { TileSourceSection } from "./components/TileSourceSection";
import {
  AppMode,
  BlendMode,
  ColorSpace,
  DistanceMetric,
  FairnessMode,
  GenerationResponse,
  LoadDestinationResponse,
  LoadTilesResponse,
  LogEntry,
  LogEvent,
  PreviewResponse,
  ProgressEvent,
  ResizeOption,
  SamplePaths,
  SortMethod,
} from "./types";

function formatNow(): string {
  const d = new Date();
  return d.toTimeString().slice(0, 8);
}

export const App: React.FC = () => {
  // Tile source state
  const [folderPath, setFolderPath] = useState("");
  const [tileCount, setTileCount] = useState(0);
  const [tileWidth, setTileWidth] = useState(50);
  const [tileHeight, setTileHeight] = useState(50);
  const [inferHeight, setInferHeight] = useState(true);
  const [resizeOpt, setResizeOpt] = useState<ResizeOption>("center");
  const [autoRotate, setAutoRotate] = useState(0);
  const [recursive, setRecursive] = useState(true);

  // Mode
  const [mode, setMode] = useState<AppMode>("mosaic");

  // Sort mode options
  const [sortMethod, setSortMethod] = useState<SortMethod>("bgr_sum");
  const [ratioW, setRatioW] = useState(16);
  const [ratioH, setRatioH] = useState(10);
  const [revRow, setRevRow] = useState(false);
  const [revSort, setRevSort] = useState(false);

  // Photomosaic options
  const [destPath, setDestPath] = useState("");
  const [destDims, setDestDims] = useState<{
    width: number;
    height: number;
    hasAlpha: boolean;
  } | null>(null);
  const [colorspace, setColorspace] = useState<ColorSpace>("lab");
  const [metric, setMetric] = useState<DistanceMetric>("euclidean");
  const [fairness, setFairness] = useState<FairnessMode>("fair");
  const [dup, setDup] = useState(2);
  const [maxWidth, setMaxWidth] = useState(80);
  const [freqMul, setFreqMul] = useState(1);
  const [dither, setDither] = useState(false);
  const [deterministic, setDeterministic] = useState(false);
  const [salient, setSalient] = useState(false);
  const [lowerThresh, setLowerThresh] = useState(0.5);
  const [transparent, setTransparent] = useState(false);
  const [blendMode, setBlendMode] = useState<BlendMode>("brightness");
  const [blendLevel, setBlendLevel] = useState(0);

  // Output / preview state
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  const [destPreviewUrl, setDestPreviewUrl] = useState<string | null>(null);
  const [hasMosaic, setHasMosaic] = useState(false);
  const [hasTileInfo, setHasTileInfo] = useState(false);
  const [outputMeta, setOutputMeta] = useState<{
    gridW?: number;
    gridH?: number;
    outW: number;
    outH: number;
    elapsedMs?: number;
  } | null>(null);

  // App UI state
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<ProgressEvent | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [samplePaths, setSamplePaths] = useState<SamplePaths | null>(null);

  const logIdRef = useRef(1);
  const blendTimerRef = useRef<number | null>(null);
  const saliencyTimerRef = useRef<number | null>(null);

  const appendLog = useCallback((level: string, message: string) => {
    setLogs((prev) => [
      ...prev.slice(-199),
      {
        id: logIdRef.current++,
        time: formatNow(),
        level,
        message,
      },
    ]);
  }, []);

  useEffect(() => {
    let unlistenProg: (() => void) | undefined;
    let unlistenLog: (() => void) | undefined;

    listen<ProgressEvent>("mosaic-progress", (event) => {
      setProgress(event.payload);
    }).then((fn) => {
      unlistenProg = fn;
    });

    listen<LogEvent>("mosaic-log", (event) => {
      appendLog(event.payload.level, event.payload.message);
    }).then((fn) => {
      unlistenLog = fn;
    });

    invoke<SamplePaths | null>("get_sample_paths")
      .then((res) => setSamplePaths(res))
      .catch(() => {});

    return () => {
      unlistenProg?.();
      unlistenLog?.();
    };
  }, [appendLog]);

  const loadTilesFromPath = useCallback(
    async (path: string) => {
      setErrorMsg(null);
      setBusy(true);
      setProgress({
        stage: "load_tiles",
        current: 0,
        total: 1,
        message: "Loading tiles...",
      });
      try {
        const res = await invoke<LoadTilesResponse>("load_tiles", {
          req: {
            folderPath: path,
            tileWidth,
            tileHeight: inferHeight ? null : tileHeight,
            inferHeight,
            resizeOpt,
            autoRotate,
            recursive,
          },
        });
        setFolderPath(path);
        setTileCount(res.tileCount);
        setTileHeight(res.tileHeight);
        setPreviewUrl(res.previewDataUrl);
        setHasMosaic(false);
        setHasTileInfo(true);
        setOutputMeta({
          gridW: res.gridWidth,
          gridH: res.gridHeight,
          outW: res.outputWidth,
          outH: res.outputHeight,
          elapsedMs: res.elapsedMs,
        });
      } catch (err) {
        const msg = String(err);
        setErrorMsg(msg);
        appendLog("error", msg);
      } finally {
        setBusy(false);
        setProgress(null);
      }
    },
    [tileWidth, tileHeight, inferHeight, resizeOpt, autoRotate, recursive, appendLog]
  );

  const handlePickFolder = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Select folder of tile images",
      defaultPath: folderPath || undefined,
    });
    if (typeof selected === "string" && selected.length > 0) {
      await loadTilesFromPath(selected);
    }
  };

  const handleReloadTiles = async () => {
    if (!folderPath) return;
    await loadTilesFromPath(folderPath);
  };

  const loadDestFromPath = useCallback(
    async (path: string) => {
      setErrorMsg(null);
      setBusy(true);
      try {
        const res = await invoke<LoadDestinationResponse>("load_destination", {
          path,
        });
        setDestPath(res.path);
        setDestDims({
          width: res.width,
          height: res.height,
          hasAlpha: res.hasAlpha,
        });
        setPreviewUrl(res.previewDataUrl);
        setDestPreviewUrl(res.previewDataUrl);
        setHasMosaic(false);
        setHasTileInfo(false);
        setOutputMeta({ outW: res.width, outH: res.height });
        if (res.hasAlpha) {
          setTransparent(true);
          setSalient(false);
          setDither(false);
        } else {
          setTransparent(false);
        }
      } catch (err) {
        const msg = String(err);
        setErrorMsg(msg);
        appendLog("error", msg);
      } finally {
        setBusy(false);
      }
    },
    [appendLog]
  );

  const handlePickDest = async () => {
    const selected = await open({
      multiple: false,
      title: "Select target destination image",
      filters: [
        {
          name: "Images",
          extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif", "tif", "tiff"],
        },
      ],
    });
    if (typeof selected === "string" && selected.length > 0) {
      await loadDestFromPath(selected);
    }
  };

  const handleLoadSamples = async () => {
    if (!samplePaths) return;
    await loadTilesFromPath(samplePaths.tilesDir);
    await loadDestFromPath(samplePaths.destImg);
    setMode("mosaic");
  };

  const triggerBlendPreview = useCallback(
    (nextMode: BlendMode, nextLevel: number) => {
      if (!hasMosaic || busy) return;
      if (blendTimerRef.current) window.clearTimeout(blendTimerRef.current);
      blendTimerRef.current = window.setTimeout(async () => {
        try {
          const res = await invoke<PreviewResponse>("apply_blend", {
            mode: nextMode,
            level: nextLevel,
          });
          setPreviewUrl(res.previewDataUrl);
        } catch (err) {
          appendLog("error", String(err));
        }
      }, 60);
    },
    [hasMosaic, busy, appendLog]
  );

  const triggerSaliencyPreview = useCallback(
    (thresh: number) => {
      if (!destDims || busy) return;
      if (saliencyTimerRef.current) window.clearTimeout(saliencyTimerRef.current);
      saliencyTimerRef.current = window.setTimeout(async () => {
        try {
          const res = await invoke<PreviewResponse>("preview_saliency", {
            threshold: thresh,
          });
          setPreviewUrl(res.previewDataUrl);
        } catch (err) {
          appendLog("error", String(err));
        }
      }, 60);
    },
    [destDims, busy, appendLog]
  );

  const handleBlendModeChange = (m: BlendMode) => {
    setBlendMode(m);
    triggerBlendPreview(m, blendLevel);
  };

  const handleBlendLevelChange = (lvl: number) => {
    setBlendLevel(lvl);
    triggerBlendPreview(blendMode, lvl);
  };

  const handleSalientChange = (enabled: boolean) => {
    setSalient(enabled);
    if (enabled && destDims && !hasMosaic) {
      triggerSaliencyPreview(lowerThresh);
    } else if (!enabled && destPreviewUrl && !hasMosaic) {
      setPreviewUrl(destPreviewUrl);
    }
  };

  const handleLowerThreshChange = (thresh: number) => {
    setLowerThresh(thresh);
    if (salient) {
      triggerSaliencyPreview(thresh);
    }
  };

  const handleTransparentChange = (enabled: boolean) => {
    setTransparent(enabled);
    if (enabled) {
      setSalient(false);
      setDither(false);
    }
  };

  const handleDitherChange = (enabled: boolean) => {
    setDither(enabled);
    if (enabled) {
      setDeterministic(true);
      setSalient(false);
      setTransparent(false);
    }
  };

  const handleGenerate = async () => {
    setErrorMsg(null);
    if (tileCount === 0) {
      setErrorMsg("Please load a folder of tiles first.");
      return;
    }

    setBusy(true);
    try {
      if (mode === "sort") {
        const res = await invoke<GenerationResponse>("generate_sorted", {
          req: {
            sortMethod,
            ratioW,
            ratioH,
            revRow,
            revSort,
          },
        });
        setPreviewUrl(res.previewDataUrl);
        setHasMosaic(false);
        setHasTileInfo(res.hasTileInfo);
        setOutputMeta({
          gridW: res.gridWidth,
          gridH: res.gridHeight,
          outW: res.outputWidth,
          outH: res.outputHeight,
          elapsedMs: res.elapsedMs,
        });
      } else {
        if (!destPath) {
          setErrorMsg("Please choose a target image for the photomosaic.");
          return;
        }
        const res = await invoke<GenerationResponse>("generate_mosaic", {
          req: {
            colorspace,
            metric,
            fairness,
            dup,
            maxWidth,
            freqMul,
            dither,
            deterministic,
            salient,
            lowerThresh,
            transparent,
            blendMode,
            blendLevel,
          },
        });
        setPreviewUrl(res.previewDataUrl);
        setHasMosaic(true);
        setHasTileInfo(res.hasTileInfo);
        setOutputMeta({
          gridW: res.gridWidth,
          gridH: res.gridHeight,
          outW: res.outputWidth,
          outH: res.outputHeight,
          elapsedMs: res.elapsedMs,
        });
      }
    } catch (err) {
      const msg = String(err);
      setErrorMsg(msg);
      appendLog("error", msg);
    } finally {
      setBusy(false);
      setProgress(null);
    }
  };

  const handleSaveImage = async () => {
    if (!previewUrl) return;
    const outPath = await save({
      title: "Save Collage / Photomosaic",
      defaultPath: "result.png",
      filters: [
        { name: "PNG Image", extensions: ["png"] },
        { name: "JPEG Image", extensions: ["jpg", "jpeg"] },
        { name: "WebP Image", extensions: ["webp"] },
      ],
    });
    if (typeof outPath === "string" && outPath.length > 0) {
      setBusy(true);
      try {
        await invoke("save_image", { path: outPath });
      } catch (err) {
        const msg = String(err);
        setErrorMsg(msg);
        appendLog("error", msg);
      } finally {
        setBusy(false);
      }
    }
  };

  const handleSaveTileInfo = async () => {
    if (!hasTileInfo) return;
    const outPath = await save({
      title: "Save Tile Grid Info",
      defaultPath: "tile_info.csv",
      filters: [
        { name: "CSV File", extensions: ["csv"] },
        { name: "Text File", extensions: ["txt"] },
      ],
    });
    if (typeof outPath === "string" && outPath.length > 0) {
      try {
        await invoke("save_tile_info", { path: outPath });
      } catch (err) {
        const msg = String(err);
        setErrorMsg(msg);
        appendLog("error", msg);
      }
    }
  };

  return (
    <div className="app-shell">
      <div className="workspace-column">
        <header className="topbar">
          <div className="brand">
            <div className="brand-icon">
              <Grid size={15} />
            </div>
            <span>Photomosaic Studio</span>
          </div>

          <div className="topbar-meta">
            {outputMeta && (
              <>
                <span className="pill">
                  {outputMeta.outW}×{outputMeta.outH} px
                </span>
                {outputMeta.gridW && outputMeta.gridH && (
                  <span className="pill">
                    grid {outputMeta.gridW}×{outputMeta.gridH}
                  </span>
                )}
                {outputMeta.elapsedMs !== undefined && (
                  <span className="pill pill-accent">
                    {(outputMeta.elapsedMs / 1000).toFixed(2)}s
                  </span>
                )}
              </>
            )}
          </div>
        </header>

        {errorMsg && (
          <div className="error-banner">
            <AlertCircle size={16} style={{ flexShrink: 0 }} />
            <span style={{ flex: 1, fontSize: 12.5 }}>{errorMsg}</span>
            <button
              className="btn-icon"
              style={{ color: "#fecaca" }}
              onClick={() => setErrorMsg(null)}
            >
              <X size={14} />
            </button>
          </div>
        )}

        <PreviewCanvas
          imageUrl={previewUrl}
          compareUrl={hasMosaic ? destPreviewUrl : null}
          busy={busy}
          progress={progress}
          hasSamples={samplePaths !== null}
          onLoadSamples={handleLoadSamples}
        />

        <LogDrawer logs={logs} onClear={() => setLogs([])} />
      </div>

      <aside className="sidebar">
        <div className="sidebar-scroll">
          <TileSourceSection
            folderPath={folderPath}
            tileCount={tileCount}
            tileWidth={tileWidth}
            tileHeight={tileHeight}
            inferHeight={inferHeight}
            resizeOpt={resizeOpt}
            autoRotate={autoRotate}
            recursive={recursive}
            busy={busy}
            onTileWidthChange={setTileWidth}
            onTileHeightChange={setTileHeight}
            onInferHeightChange={(v) => {
              setInferHeight(v);
              if (!v && tileHeight < 2) setTileHeight(tileWidth);
            }}
            onResizeOptChange={setResizeOpt}
            onAutoRotateChange={setAutoRotate}
            onRecursiveChange={setRecursive}
            onPickFolder={handlePickFolder}
            onReload={handleReloadTiles}
          />

          <div className="segmented">
            <button
              className={`segmented-btn ${mode === "mosaic" ? "active" : ""}`}
              onClick={() => setMode("mosaic")}
            >
              <Sparkles size={13} />
              Photomosaic
            </button>
            <button
              className={`segmented-btn ${mode === "sort" ? "active" : ""}`}
              onClick={() => setMode("sort")}
            >
              <LayoutGrid size={13} />
              Sort Collage
            </button>
          </div>

          {mode === "mosaic" ? (
            <MosaicOptionsSection
              destPath={destPath}
              destDims={destDims}
              colorspace={colorspace}
              metric={metric}
              fairness={fairness}
              dup={dup}
              maxWidth={maxWidth}
              freqMul={freqMul}
              dither={dither}
              deterministic={deterministic}
              salient={salient}
              lowerThresh={lowerThresh}
              transparent={transparent}
              blendMode={blendMode}
              blendLevel={blendLevel}
              busy={busy}
              onPickDest={handlePickDest}
              onColorspaceChange={setColorspace}
              onMetricChange={setMetric}
              onFairnessChange={setFairness}
              onDupChange={setDup}
              onMaxWidthChange={setMaxWidth}
              onFreqMulChange={setFreqMul}
              onDitherChange={handleDitherChange}
              onDeterministicChange={setDeterministic}
              onSalientChange={handleSalientChange}
              onLowerThreshChange={handleLowerThreshChange}
              onTransparentChange={handleTransparentChange}
              onBlendModeChange={handleBlendModeChange}
              onBlendLevelChange={handleBlendLevelChange}
            />
          ) : (
            <SortOptionsSection
              sortMethod={sortMethod}
              ratioW={ratioW}
              ratioH={ratioH}
              revRow={revRow}
              revSort={revSort}
              onSortMethodChange={setSortMethod}
              onRatioWChange={setRatioW}
              onRatioHChange={setRatioH}
              onRevRowChange={setRevRow}
              onRevSortChange={setRevSort}
            />
          )}
        </div>

        <footer className="sidebar-footer">
          <button
            className="btn btn-primary"
            onClick={handleGenerate}
            disabled={busy || tileCount === 0 || (mode === "mosaic" && !destPath)}
          >
            {busy ? (
              <>
                <Loader2 size={15} className="spin" />
                Processing...
              </>
            ) : (
              <>
                <Play size={15} />
                {mode === "mosaic" ? "Generate Photomosaic" : "Generate Sorted Collage"}
              </>
            )}
          </button>

          <div className="btn-row-equal">
            <button
              className="btn"
              onClick={handleSaveImage}
              disabled={busy || !previewUrl}
            >
              <Download size={14} />
              Save Image
            </button>
            <button
              className="btn"
              onClick={handleSaveTileInfo}
              disabled={busy || !hasTileInfo}
            >
              <FileSpreadsheet size={14} />
              Tile Info
            </button>
          </div>
        </footer>
      </aside>
    </div>
  );
};

export default App;
