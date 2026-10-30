"""Génère un modèle ONNX jouet (sortie = sigmoïde de l'obscurité moyenne) pour tester la chaîne d'inférence.

Usage : python make_toy_model.py sortie.onnx   (nécessite `pip install onnx numpy`)
"""
import sys

import numpy as np
import onnx
from onnx import TensorProto, helper, numpy_helper

x = helper.make_tensor_value_info("input", TensorProto.FLOAT, [1, 3, 320, 320])
y = helper.make_tensor_value_info("mask", TensorProto.FLOAT, [1, 1, 320, 320])
two = numpy_helper.from_array(np.array(2.0, dtype=np.float32), "two")
nodes = [
    helper.make_node("ReduceMean", ["input"], ["m"], axes=[1], keepdims=1),
    helper.make_node("Neg", ["m"], ["n"]),
    helper.make_node("Mul", ["n", "two"], ["s"]),
    helper.make_node("Sigmoid", ["s"], ["mask"]),
]
model = helper.make_model(helper.make_graph(nodes, "toy", [x], [y], [two]), opset_imports=[helper.make_opsetid("", 13)])
model.ir_version = 8
onnx.checker.check_model(model)
onnx.save(model, sys.argv[1])
