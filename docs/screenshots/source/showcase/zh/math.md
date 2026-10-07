# 斯托克斯定理

**斯托克斯定理**指出，只要知道向量场 $\mathbf{F}$ 沿曲面 $S$ 边界的取值，就能计算旋度 $\operatorname{curl}\mathbf{F}$ 穿过 $S$ 的通量。反过来，也可以将向量场 $\mathbf{F}$ 沿曲面 $S$ 边界的线积分，转化为它的旋度在 $S$ 上的二重积分。

设 $S$ 是一个有向光滑曲面，单位法向量为 $\mathbf{N}$。再假设 $S$ 的边界是一条简单闭曲线 $C$。如果沿 $C$ 的正方向行走，头部指向 $\mathbf{N}$ 的方向时，曲面始终在左侧，那么 $S$ 的定向就诱导了 $C$ 的正向。有了这个定义，就可以陈述斯托克斯定理。

## 定理 6.19：斯托克斯定理

设 $S$ 是一个分片光滑的有向曲面，其边界是一条具有正向的简单闭曲线 $C$（见[图 6.79](https://openstax.org/books/calculus-volume-3/pages/6-7-stokes-theorem)）。如果向量场 $\mathbf{F}$ 的各分量函数在包含 $S$ 的某个开区域内具有连续偏导数，那么

$$
\int_C \mathbf{F}\cdot d\mathbf{r}
=\iint_S \operatorname{curl}\mathbf{F}\cdot d\mathbf{S}.
$$

假设曲面 $S$ 是 $xy$ 平面上的一个平面区域，取向向上。此时单位法向量为 $\mathbf{k}$，曲面积分 $\iint_S\operatorname{curl}\mathbf{F}\cdot d\mathbf{S}$ 实际上就是二重积分 $\iint_S\operatorname{curl}\mathbf{F}\cdot\mathbf{k}\,dA$。在这个特殊情形下，斯托克斯定理给出

$$
\int_C\mathbf{F}\cdot d\mathbf{r}
=\iint_S\operatorname{curl}\mathbf{F}\cdot\mathbf{k}\,dA.
$$

这就是格林定理的环流形式，说明格林定理是斯托克斯定理的一个特例。格林定理只能处理平面上的曲面，而斯托克斯定理既能处理平面上的曲面，也能处理空间中的曲面。

斯托克斯定理的完整证明超出了本书的范围。我们先直观地解释为什么这个定理成立，然后在一个特殊情形下证明它：曲面 $S$ 是某个函数图像的一部分，且 $S$、它的边界和 $\mathbf{F}$ 都满足较好的条件。

## 证明

首先来看一个非正式的证明。它并不严格，但能帮助我们大致理解定理为什么成立。设 $S$ 是一个曲面，$D$ 是其中的一小块，且与 $S$ 的边界没有公共点。取足够小的 $D$，使它可以用一个有向正方形 $E$ 来近似。让 $D$ 继承 $S$ 的定向，并让 $E$ 具有同样的定向。

这个正方形有四条边，分别用 $E_l$、$E_r$、$E_u$ 和 $E_d$ 表示左、右、上、下四边。对这个正方形，可以使用格林定理的通量形式：

$$
\int_{E_l+E_d+E_r+E_u}\mathbf{F}\cdot d\mathbf{r}
=\iint_E\operatorname{curl}\mathbf{F}\cdot\mathbf{N}\,dS
=\iint_E\operatorname{curl}\mathbf{F}\cdot d\mathbf{S}.
$$

为了近似整个曲面上的通量，把近似各小块曲面的正方形上的通量相加（见[图 6.80](https://openstax.org/books/calculus-volume-3/pages/6-7-stokes-theorem)）。由格林定理，每个正方形上的通量都等于沿它边界的线积分。设 $F$ 是另一个近似正方形，它继承了曲面的定向，其右边与 $E_l$ 重合，也就是说，$F$ 位于 $E$ 左侧。

用 $F_r$ 表示 $F$ 的右边，则 $E_l=-F_r$。换言之，$F$ 的右边与 $E$ 的左边是同一条曲线，但方向相反。因此，

$$
\int_{E_l}\mathbf{F}\cdot d\mathbf{r}
=-\int_{F_r}\mathbf{F}\cdot d\mathbf{r}.
$$
