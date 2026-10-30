// Remplit la page de téléchargement depuis /pics/releases.json (généré par le script de déploiement).
// Sans JS ou sans release publiée, la page reste lisible et n'affiche aucun lien factice.
(function () {
  var list = document.getElementById("downloads");
  if (!list) return;
  fetch("releases.json", { cache: "no-cache" })
    .then(function (r) { if (!r.ok) throw new Error(r.status); return r.json(); })
    .then(function (d) {
      if (!d.files || !d.files.length) return;
      document.getElementById("version").textContent = d.version;
      var sums = document.getElementById("sums");
      list.textContent = "";
      d.files.forEach(function (f) {
        var li = document.createElement("li");
        var left = document.createElement("div");
        left.innerHTML = "<strong></strong><br><small></small>";
        left.querySelector("strong").textContent = f.label;
        left.querySelector("small").textContent = f.name + " · " + (f.size / 1048576).toFixed(1) + " Mo";
        var a = document.createElement("a");
        a.className = "btn"; a.href = "download/" + encodeURIComponent(f.name);
        a.textContent = "Télécharger";
        li.appendChild(left); li.appendChild(a); list.appendChild(li);
        var tr = document.createElement("tr");
        tr.innerHTML = "<td></td><td><code></code></td>";
        tr.children[0].textContent = f.name; tr.querySelector("code").textContent = f.sha256;
        sums.appendChild(tr);
      });
      document.getElementById("sums-wrap").hidden = false;
    })
    .catch(function () { /* aucune release : l'état par défaut du HTML s'applique */ });
})();
