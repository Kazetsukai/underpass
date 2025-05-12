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

  checkState();
  setInterval(checkState, 3000);
});
