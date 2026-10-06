import "./styles.css";


type Measurement = {
  wavelength: number;
  frequency: number;
  intensity: number;
  transmission: number;
};


const SPEED_OF_LIGHT =
  299_792_458;


let connected = false;

let scanning = false;

let scanNumber = 0;

let currentWavelength = 400;

let scanTimer:
  number | undefined;


let measurements:
  Measurement[] = [];


let calibrationIntensity =
  1000;


// ------------------------------------
// HTML ELEMENTS
// ------------------------------------

const connectionStatus =
  document.querySelector(
    "#connectionStatus"
  ) as HTMLDivElement;


const connectButton =
  document.querySelector(
    "#connectButton"
  ) as HTMLButtonElement;


const startButton =
  document.querySelector(
    "#startButton"
  ) as HTMLButtonElement;


const stopButton =
  document.querySelector(
    "#stopButton"
  ) as HTMLButtonElement;


const calibrateButton =
  document.querySelector(
    "#calibrateButton"
  ) as HTMLButtonElement;


const saveButton =
  document.querySelector(
    "#saveButton"
  ) as HTMLButtonElement;


const wavelengthValue =
  document.querySelector(
    "#wavelengthValue"
  ) as HTMLElement;


const frequencyValue =
  document.querySelector(
    "#frequencyValue"
  ) as HTMLElement;


const intensityValue =
  document.querySelector(
    "#intensityValue"
  ) as HTMLElement;


const transmissionValue =
  document.querySelector(
    "#transmissionValue"
  ) as HTMLElement;


const scanNumberText =
  document.querySelector(
    "#scanNumber"
  ) as HTMLElement;


const pointCount =
  document.querySelector(
    "#pointCount"
  ) as HTMLElement;


const scanState =
  document.querySelector(
    "#scanState"
  ) as HTMLElement;


const measurementTable =
  document.querySelector(
    "#measurementTable"
  ) as HTMLTableSectionElement;


const canvas =
  document.querySelector(
    "#spectrumChart"
  ) as HTMLCanvasElement;


const ctx =
  canvas.getContext("2d")!;


// ------------------------------------
// CALCULATIONS
// ------------------------------------

function wavelengthToFrequency(
  wavelengthNm: number
): number {

  const wavelengthMeters =
    wavelengthNm * 1e-9;

  const frequencyHz =
    SPEED_OF_LIGHT
    / wavelengthMeters;

  return frequencyHz / 1e12;
}


// ------------------------------------
// SIMULATED SENSOR
// ------------------------------------

function simulateIntensity(
  wavelength: number
): number {

  const peak1 =
    300
    * Math.exp(
      -Math.pow(
        wavelength - 520,
        2
      )
      / 6000
    );


  const peak2 =
    220
    * Math.exp(
      -Math.pow(
        wavelength - 740,
        2
      )
      / 9000
    );


  const wave =
    80
    * Math.sin(
      wavelength / 70
    );


  const noise =
    Math.random()
    * 35;


  return Math.max(
    50,
    420
    + peak1
    + peak2
    + wave
    + noise
  );
}


// ------------------------------------
// CONNECT
// ------------------------------------

connectButton.addEventListener(
  "click",
  () => {

    connected =
      !connected;


    if (connected) {

      connectionStatus
        .classList
        .remove(
          "disconnected"
        );

      connectionStatus
        .classList
        .add(
          "connected"
        );

      connectionStatus.innerHTML =
        `
        <span
          class="status-dot">
        </span>

        Connected
        `;


      connectButton.textContent =
        "Disconnect Sensor";

    } else {

      stopScan();


      connectionStatus
        .classList
        .remove(
          "connected"
        );

      connectionStatus
        .classList
        .add(
          "disconnected"
        );

      connectionStatus.innerHTML =
        `
        <span
          class="status-dot">
        </span>

        Disconnected
        `;


      connectButton.textContent =
        "Connect Sensor";
    }
  }
);


// ------------------------------------
// START SCAN
// ------------------------------------

startButton.addEventListener(
  "click",
  () => {

    if (!connected) {

      alert(
        "Connect the sensor first."
      );

      return;
    }


    if (scanning) {
      return;
    }


    scanning = true;

    scanNumber++;

    currentWavelength =
      400;

    measurements = [];


    scanNumberText.textContent =
      `#${scanNumber}`;


    scanState.textContent =
      "Scanning";


    scanTimer =
      window.setInterval(
        captureMeasurement,
        80
      );
  }
);


// ------------------------------------
// CAPTURE ONE DATA POINT
// ------------------------------------

function captureMeasurement() {

  if (
    !scanning
  ) {
    return;
  }


  if (
    currentWavelength
    > 940
  ) {

    stopScan();

    scanState.textContent =
      "Complete";

    return;
  }


  const intensity =
    simulateIntensity(
      currentWavelength
    );


  const frequency =
    wavelengthToFrequency(
      currentWavelength
    );


  const transmission =
    Math.min(
      100,
      intensity
      / calibrationIntensity
      * 100
    );


  const measurement:
    Measurement =
  {
    wavelength:
      currentWavelength,

    frequency,

    intensity,

    transmission
  };


  measurements.push(
    measurement
  );


  updateCurrentReading(
    measurement
  );


  updateTable();

  drawGraph();


  pointCount.textContent =
    measurements
      .length
      .toString();


  currentWavelength +=
    10;
}


// ------------------------------------
// STOP SCAN
// ------------------------------------

stopButton.addEventListener(
  "click",
  stopScan
);


function stopScan() {

  scanning = false;


  if (
    scanTimer !== undefined
  ) {

    clearInterval(
      scanTimer
    );

    scanTimer =
      undefined;
  }


  if (
    measurements.length
    > 0
  ) {

    scanState.textContent =
      "Stopped";

  } else {

    scanState.textContent =
      "Ready";
  }
}


// ------------------------------------
// CALIBRATION
// ------------------------------------

calibrateButton.addEventListener(
  "click",
  () => {

    calibrationIntensity =
      1000;


    alert(
      "Calibration reference saved."
    );
  }
);


// ------------------------------------
// UPDATE CURRENT DISPLAY
// ------------------------------------

function updateCurrentReading(
  measurement: Measurement
) {

  wavelengthValue.textContent =
    `${measurement.wavelength.toFixed(
      0
    )} nm`;


  frequencyValue.textContent =
    `${measurement.frequency.toFixed(
      2
    )} THz`;


  intensityValue.textContent =
    measurement.intensity.toFixed(
      0
    );


  transmissionValue.textContent =
    `${measurement.transmission.toFixed(
      1
    )} %`;
}


// ------------------------------------
// TABLE
// ------------------------------------

function updateTable() {

  const latest =
    measurements
      .slice(-7)
      .reverse();


  measurementTable.innerHTML =
    "";


  latest.forEach(
    (measurement) => {

      const row =
        document.createElement(
          "tr"
        );


      row.innerHTML =
        `
        <td>
          ${measurement.wavelength.toFixed(
            0
          )} nm
        </td>

        <td>
          ${measurement.frequency.toFixed(
            2
          )} THz
        </td>

        <td>
          ${measurement.intensity.toFixed(
            0
          )}
        </td>

        <td>
          ${measurement.transmission.toFixed(
            1
          )} %
        </td>
        `;


      measurementTable
        .appendChild(
          row
        );
    }
  );
}


// ------------------------------------
// DRAW GRAPH
// ------------------------------------

function drawGraph() {

  resizeCanvas();


  const width =
    canvas.width;

  const height =
    canvas.height;


  ctx.clearRect(
    0,
    0,
    width,
    height
  );


  const left =
    65;

  const right =
    width - 25;

  const top =
    25;

  const bottom =
    height - 45;


  // GRID
  ctx.strokeStyle =
    "#263342";

  ctx.lineWidth =
    1;


  for (
    let i = 0;
    i <= 5;
    i++
  ) {

    const y =
      top
      + (
        bottom
        - top
      )
      * i
      / 5;


    ctx.beginPath();

    ctx.moveTo(
      left,
      y
    );

    ctx.lineTo(
      right,
      y
    );

    ctx.stroke();
  }


  // X LABELS
  ctx.fillStyle =
    "#758396";

  ctx.font =
    "12px Arial";


  const wavelengths =
    [
      400,
      500,
      600,
      700,
      800,
      900,
      940
    ];


  wavelengths.forEach(
    wavelength => {

      const x =
        left
        + (
          wavelength
          - 400
        )
        / 540
        * (
          right
          - left
        );


      ctx.fillText(
        wavelength.toString(),
        x - 12,
        bottom + 25
      );
    }
  );


  ctx.fillText(
    "Wavelength (nm)",
    width / 2 - 45,
    height - 8
  );


  // NO DATA YET
  if (
    measurements.length
    === 0
  ) {

    ctx.fillStyle =
      "#657487";

    ctx.font =
      "16px Arial";


    ctx.fillText(
      "Start a scan to display spectral data",
      width / 2 - 140,
      height / 2
    );

    return;
  }


  const maxIntensity =
    Math.max(
      ...measurements.map(
        measurement =>
          measurement.intensity
      ),
      1000
    );


  // GRAPH LINE
  ctx.beginPath();


  measurements.forEach(
    (
      measurement,
      index
    ) => {

      const x =
        left
        + (
          measurement.wavelength
          - 400
        )
        / 540
        * (
          right
          - left
        );


      const y =
        bottom
        - (
          measurement.intensity
          / maxIntensity
        )
        * (
          bottom
          - top
        );


      if (
        index === 0
      ) {

        ctx.moveTo(
          x,
          y
        );

      } else {

        ctx.lineTo(
          x,
          y
        );
      }
    }
  );


  ctx.strokeStyle =
    "#4287ff";

  ctx.lineWidth =
    3;

  ctx.stroke();


  // DATA POINTS
  measurements.forEach(
    measurement => {

      const x =
        left
        + (
          measurement.wavelength
          - 400
        )
        / 540
        * (
          right
          - left
        );


      const y =
        bottom
        - (
          measurement.intensity
          / maxIntensity
        )
        * (
          bottom
          - top
        );


      ctx.beginPath();

      ctx.arc(
        x,
        y,
        3,
        0,
        Math.PI * 2
      );

      ctx.fillStyle =
        "#76a9ff";

      ctx.fill();
    }
  );
}


// ------------------------------------
// RESIZE GRAPH
// ------------------------------------

function resizeCanvas() {

  const rect =
    canvas
      .getBoundingClientRect();


  canvas.width =
    rect.width
    * window.devicePixelRatio;


  canvas.height =
    rect.height
    * window.devicePixelRatio;


  ctx.setTransform(
    window.devicePixelRatio,
    0,
    0,
    window.devicePixelRatio,
    0,
    0
  );


  canvas.width =
    rect.width;

  canvas.height =
    rect.height;
}


window.addEventListener(
  "resize",
  drawGraph
);


// ------------------------------------
// SAVE CSV
// ------------------------------------

saveButton.addEventListener(
  "click",
  () => {

    if (
      measurements.length
      === 0
    ) {

      alert(
        "Run a scan before saving."
      );

      return;
    }


    let csv =
      "wavelength_nm,frequency_thz,intensity,transmission_percent\n";


    measurements.forEach(
      measurement => {

        csv +=
          `${measurement.wavelength},`
          + `${measurement.frequency.toFixed(
            4
          )},`
          + `${measurement.intensity.toFixed(
            2
          )},`
          + `${measurement.transmission.toFixed(
            2
          )}\n`;
      }
    );


    const blob =
      new Blob(
        [csv],
        {
          type:
            "text/csv"
        }
      );


    const url =
      URL.createObjectURL(
        blob
      );


    const link =
      document.createElement(
        "a"
      );


    link.href =
      url;


    link.download =
      `spectral_scan_${scanNumber}.csv`;


    link.click();


    URL.revokeObjectURL(
      url
    );
  }
);


// INITIAL GRAPH
drawGraph();