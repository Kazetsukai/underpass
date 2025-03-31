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

document.addEventListener("DOMContentLoaded", function () {
  const lightingToggle = document.querySelector("#lightingToggle");

  function checkState() {
    fetch("./state")
      .then((response) => response.json())
      .then((data) => {
        lightingToggle.checked = data;
      });
  }

  lightingToggle.addEventListener(
    "click",
    debounce_leading(function () {
      fetch("./power", { method: "POST" })
        .then((a) => a.json())
        .then((state) => {
          lightingToggle.checked = state;
        });
      return false;
    })
  );
});
