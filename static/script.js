// Ignore calls for a short time after first call
function debounce_leading(func, timeout = 300) {
  let timer;
  return (...args) => {
    if (!timer) {
      return func(...args);
    }
    clearTimeout(timer);
    timer = setTimeout(() => {
      timer = undefined;
    }, timeout);

    return false;
  };
}

const modeMap = {
  Off: 0,
  On: 1,
  Flickering: 2,
};

function enumToString(enumValue) {
  if (typeof enumValue === "string") return enumValue;
  if (typeof enumValue === "object") {
    return Object.keys(enumValue)[0];
  }
  return "";
}

document.addEventListener("DOMContentLoaded", function () {
  const lightingToggle = document.querySelector("#lightingToggle");
  const modesGrid = document.querySelector("#modesGrid");
  const streetlampModes = [0, 1, 2, 3, 4, 5].map((n) => {
    const label = document.createElement("label");
    const mode = document.createElement("select");
    mode.id = `mode${n}`;
    mode.name = `mode${n}`;
    mode.innerHTML = `
      <option value="0">Off</option>
      <option value="1">On</option>
      <option value="2">Flicker</option>
    `;
    mode.value = 0;
    mode.addEventListener("change", function () {
      fetch(`./lamp/${n}/${mode.value}`, {
        method: "POST",
        body: "",
        headers: {
          "Content-Type": "application/json",
        },
      });
    });

    label.appendChild(document.createTextNode(`Lamp #${n}`));
    label.appendChild(mode);

    modesGrid.appendChild(label);
    return mode;
  });

  function checkState() {
    fetch("./state")
      .then((response) => response.json())
      .then((data) => {
        lightingToggle.checked = data.streetlamps_enabled;
        streetlampModes.forEach((mode, index) => {
          mode.value = modeMap[enumToString(data.streetlamps_modes[index])];
        });
        updateUnderpassState(data);
      });
  }

  lightingToggle.addEventListener(
    "click",
    debounce_leading(function () {
      fetch("./power", { method: "POST" })
        .then((a) => a.json())
        .then(() => {
          checkState();
        });
      return false;
    })
  );

  function rgbToHex(rgb) {
    return (
      "#" +
      rgb.r.toString(16).padStart(2, "0") +
      rgb.g.toString(16).padStart(2, "0") +
      rgb.b.toString(16).padStart(2, "0")
    );
  }

  function hexToRgb(hex) {
    hex = hex.replace("#", "");
    if (hex.length === 3) {
      hex = hex
        .split("")
        .map((h) => h + h)
        .join("");
    }
    const num = parseInt(hex, 16);
    return { r: (num >> 16) & 0xff, g: (num >> 8) & 0xff, b: num & 0xff };
  }

  let dirty = true;
  function renderUnderpassParams(state) {
    const paramsDiv = document.getElementById("underpassModeParams");
    paramsDiv.innerHTML = "";
    const mode = state.underpass_lights_state;
    if (mode.SingleColour) {
      // Default color
      const color = { r: 40, g: 20, b: 2 };
      const label = document.createElement("label");
      label.textContent = "Colour: ";
      const input = document.createElement("input");
      input.type = "color";
      input.id = "underpassColour";
      input.value = rgbToHex(color);
      label.appendChild(input);
      paramsDiv.appendChild(label);
    } else if (mode.Cars) {
      // Cars mode
      const cars = mode.Cars;
      // Default color
      const labelColor = document.createElement("label");
      labelColor.textContent = "Default Colour: ";
      const inputColor = document.createElement("input");
      inputColor.type = "color";
      inputColor.id = "underpassCarsColour";
      inputColor.value = rgbToHex(cars.default_color);
      labelColor.appendChild(inputColor);
      paramsDiv.appendChild(labelColor);
      // Min interval
      const labelMin = document.createElement("label");
      labelMin.textContent = "Min Interval: ";
      const inputMin = document.createElement("input");
      inputMin.type = "number";
      inputMin.id = "underpassCarsMin";
      inputMin.value = cars.min_interval;
      labelMin.appendChild(inputMin);
      paramsDiv.appendChild(labelMin);
      // Max interval
      const labelMax = document.createElement("label");
      labelMax.textContent = "Max Interval: ";
      const inputMax = document.createElement("input");
      inputMax.type = "number";
      inputMax.id = "underpassCarsMax";
      inputMax.value = cars.max_interval;
      labelMax.appendChild(inputMax);
      paramsDiv.appendChild(labelMax);
      // Speed limit
      const labelSpeed = document.createElement("label");
      labelSpeed.textContent = "Speed Limit (kph): ";
      const inputSpeed = document.createElement("input");
      inputSpeed.type = "number";
      inputSpeed.id = "underpassCarsSpeed";
      inputSpeed.value = cars.speed_limit_kph;
      labelSpeed.appendChild(inputSpeed);
      paramsDiv.appendChild(labelSpeed);
    }
  }

  function getUnderpassModeValue(mode) {
    if (typeof mode === "string") return mode;
    if (typeof mode === "object") {
      if (mode.SingleColour) return "SingleColour";
      if (mode.RainbowCycle) return "RainbowCycle";
      if (mode.Cars) return "Cars";
    }
    return "Off";
  }

  const underpassMode = document.getElementById("underpassMode");
  const underpassParams = document.getElementById("underpassModeParams");

  function updateUnderpassState(state) {
    // Set mode dropdown
    const modeVal = getUnderpassModeValue(state.underpass_lights_state);
    if (underpassMode.value !== modeVal || dirty) {
      underpassMode.value = modeVal;
      renderUnderpassParams(state);
      dirty = false;
    }
    // Set param values if present
    if (modeVal === "SingleColour") {
      const input = document.getElementById("underpassColour");
      if (
        input &&
        typeof state.underpass_lights_state === "object" &&
        state.underpass_lights_state.SingleColour
      ) {
        input.value = rgbToHex(state.underpass_lights_state.SingleColour);
      }
    } else if (modeVal === "Cars") {
      const cars = state.underpass_lights_state.Cars;
      if (cars) {
        document.getElementById("underpassCarsColour").value = rgbToHex(
          cars.default_color
        );
        document.getElementById("underpassCarsMin").value = cars.min_interval;
        document.getElementById("underpassCarsMax").value = cars.max_interval;
        document.getElementById("underpassCarsSpeed").value =
          cars.speed_limit_kph;
      }
    }
  }

  function updateUnderpassConfig() {
    fetch("./state")
      .then((response) => response.json())
      .then((data) => {
        let newState = { ...data };
        const mode = underpassMode.value;
        if (mode === "Off" || mode === "RainbowCycle") {
          newState.underpass_lights_state = mode;
        } else if (mode === "SingleColour") {
          let color = { r: 40, g: 20, b: 2 };
          const colorInput = document.getElementById("underpassColour");
          if (colorInput) {
            color = hexToRgb(colorInput.value);
          }
          newState.underpass_lights_state = { SingleColour: color };
        } else if (mode === "Cars") {
          let default_color = { r: 40, g: 20, b: 2 };
          let min_interval = 10;
          let max_interval = 30;
          let speed_limit_kph = 30;
          const colorInput = document.getElementById("underpassCarsColour");
          const minInput = document.getElementById("underpassCarsMin");
          const maxInput = document.getElementById("underpassCarsMax");
          const speedInput = document.getElementById("underpassCarsSpeed");
          if (colorInput) default_color = hexToRgb(colorInput.value);
          if (minInput) min_interval = parseInt(minInput.value);
          if (maxInput) max_interval = parseInt(maxInput.value);
          if (speedInput) speed_limit_kph = parseInt(speedInput.value);
          newState.underpass_lights_state = {
            Cars: {
              default_color,
              min_interval,
              max_interval,
              speed_limit_kph,
            },
          };
        }
        dirty = true;
        fetch("./state", {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(newState),
        }).then(() => {
          checkState();
        });
      });
  }

  checkState();
  setInterval(checkState, 10000);

  underpassMode.addEventListener("change", updateUnderpassConfig);
  underpassParams.addEventListener("change", updateUnderpassConfig);
});
